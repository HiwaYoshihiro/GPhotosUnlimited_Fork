//! String creation and local-reference cleanup.
//!
//! `inject` has many early returns (missing class, missing method, pending
//! exception, stale dex). Each one used to leak every local reference created
//! up to that point. These tests drive the guard directly, since a host has no
//! JNI environment. The `new_string` tests use a hand-built function table for
//! the same reason.

use core::mem;
use std::cell::RefCell;

use jni_sys::{jchar, jobject, jsize, jstring, JNIEnv, JNINativeInterface_};
use unlimitedphotos::jni::{new_string, LocalRefs};

thread_local! {
    static DELETED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static CAPTURED: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static CLEARED: RefCell<u32> = const { RefCell::new(0) };
    static PENDING: RefCell<bool> = const { RefCell::new(false) };
    static RETURN_NULL: RefCell<bool> = const { RefCell::new(false) };
}

/// A non-null `jstring` that is never dereferenced: the fake table only records
/// the call, so any distinguishable pointer works.
const FAKE_STRING: usize = 0x5eed;

unsafe extern "system" fn record_delete(_env: *mut JNIEnv, obj: jobject) {
    DELETED.with(|deleted| deleted.borrow_mut().push(obj as usize));
}

fn fake_ref(id: usize) -> jobject {
    id as jobject
}

fn deleted() -> Vec<usize> {
    DELETED.with(|deleted| deleted.borrow().clone())
}

fn clear() {
    DELETED.with(|deleted| deleted.borrow_mut().clear());
    CAPTURED.with(|captured| captured.borrow_mut().clear());
    CLEARED.with(|cleared| *cleared.borrow_mut() = 0);
    PENDING.with(|pending| *pending.borrow_mut() = false);
    RETURN_NULL.with(|null| *null.borrow_mut() = false);
}

#[test]
fn every_reference_is_deleted_on_drop() {
    clear();
    {
        let mut refs = LocalRefs::with_deleter(core::ptr::null_mut(), Some(record_delete));
        refs.push(fake_ref(1));
        refs.push(fake_ref(2));
        refs.push(fake_ref(3));
        assert!(deleted().is_empty(), "nothing is deleted while alive");
    }
    assert_eq!(deleted(), vec![1, 2, 3]);
}

#[test]
fn dropping_early_still_deletes_what_was_taken() {
    clear();
    {
        // Stands in for an early `return` in `inject`.
        let mut refs = LocalRefs::with_deleter(core::ptr::null_mut(), Some(record_delete));
        refs.push(fake_ref(7));
        refs.push(fake_ref(8));
    }
    assert_eq!(deleted(), vec![7, 8]);
}

#[test]
fn null_references_are_not_registered() {
    clear();
    {
        let mut refs = LocalRefs::with_deleter(core::ptr::null_mut(), Some(record_delete));
        refs.push(core::ptr::null_mut());
        refs.push(fake_ref(4));
        refs.push(core::ptr::null_mut());
    }
    assert_eq!(deleted(), vec![4]);
}

#[test]
fn an_empty_guard_deletes_nothing() {
    clear();
    {
        let _refs = LocalRefs::with_deleter(core::ptr::null_mut(), Some(record_delete));
    }
    assert!(deleted().is_empty());
}

#[test]
fn a_missing_deleter_is_not_a_crash() {
    // `DeleteLocalRef` is optional in the JNI table; if the platform omits it
    // the guard must still be safe to drop.
    {
        let mut refs = LocalRefs::with_deleter(core::ptr::null_mut(), None);
        refs.push(fake_ref(9));
    }
}

unsafe extern "system" fn record_new_string(
    _env: *mut JNIEnv,
    unicode: *const jchar,
    len: jsize,
) -> jstring {
    let units = core::slice::from_raw_parts(unicode, len as usize);
    CAPTURED.with(|captured| captured.borrow_mut().extend_from_slice(units));
    if RETURN_NULL.with(|null| *null.borrow()) {
        core::ptr::null_mut()
    } else {
        FAKE_STRING as jstring
    }
}

unsafe extern "system" fn fake_exception_check(_env: *mut JNIEnv) -> jni_sys::jboolean {
    PENDING.with(|pending| *pending.borrow()) as jni_sys::jboolean
}

unsafe extern "system" fn record_exception_clear(_env: *mut JNIEnv) {
    CLEARED.with(|cleared| *cleared.borrow_mut() += 1);
    PENDING.with(|pending| *pending.borrow_mut() = false);
}

/// Builds a minimal JNI table. Every field not set here stays null, which is
/// what the code under test must tolerate.
struct FakeEnv {
    table: Box<JNINativeInterface_>,
    /// `JNIEnv` is itself a pointer to the function table, so the caller needs
    /// a stable slot holding that pointer.
    slot: Box<JNIEnv>,
}

impl FakeEnv {
    fn new(with_new_string: bool) -> Self {
        clear();
        let mut table: JNINativeInterface_ = unsafe { mem::zeroed() };
        table.ExceptionCheck = Some(fake_exception_check);
        table.ExceptionClear = Some(record_exception_clear);
        if with_new_string {
            table.NewString = Some(record_new_string);
        }
        let table = Box::new(table);
        let slot = Box::new(&*table as *const JNINativeInterface_ as JNIEnv);
        Self { table, slot }
    }

    fn env(&self) -> *mut JNIEnv {
        let _ = &self.table;
        &*self.slot as *const JNIEnv as *mut JNIEnv
    }
}

fn captured() -> Vec<u16> {
    CAPTURED.with(|captured| captured.borrow().clone())
}

fn cleared() -> u32 {
    CLEARED.with(|cleared| *cleared.borrow())
}

#[test]
fn ascii_text_is_passed_as_utf16() {
    let fake = FakeEnv::new(true);
    let string = unsafe { new_string(fake.env(), "hello") };
    assert_eq!(string, Some(FAKE_STRING as jstring));
    assert_eq!(captured(), "hello".encode_utf16().collect::<Vec<_>>());
    assert_eq!(cleared(), 0);
}

#[test]
fn a_character_outside_the_bmp_becomes_a_surrogate_pair() {
    // The whole point of using `NewString`: modified UTF-8 would encode this as
    // two three-byte surrogate escapes, and plain UTF-8 as one four-byte
    // sequence, which `NewStringUTF` does not accept.
    let fake = FakeEnv::new(true);
    let string = unsafe { new_string(fake.env(), "📷") };
    assert_eq!(string, Some(FAKE_STRING as jstring));
    assert_eq!(captured(), vec![0xd83d, 0xdcf7]);
}

#[test]
fn a_failed_allocation_clears_the_exception() {
    // `NewString` reports OOM by returning null and leaving an exception
    // pending. Returning without clearing it aborts the next JNI call on this
    // thread once `inject` hands control back to Zygisk.
    let fake = FakeEnv::new(true);
    RETURN_NULL.with(|null| *null.borrow_mut() = true);
    PENDING.with(|pending| *pending.borrow_mut() = true);

    let string = unsafe { new_string(fake.env(), "hello") };

    assert!(string.is_none());
    assert_eq!(cleared(), 1, "the pending exception must be cleared");
    assert!(!PENDING.with(|pending| *pending.borrow()));
}

#[test]
fn a_missing_new_string_is_not_a_crash() {
    let fake = FakeEnv::new(false);
    assert!(unsafe { new_string(fake.env(), "hello") }.is_none());
}

#[test]
fn a_null_environment_is_not_a_crash() {
    assert!(unsafe { new_string(core::ptr::null_mut(), "hello") }.is_none());
}
