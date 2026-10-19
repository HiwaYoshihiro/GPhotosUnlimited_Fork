//! Local-reference cleanup.
//!
//! `inject` has many early returns (missing class, missing method, pending
//! exception, stale dex). Each one used to leak every local reference created
//! up to that point. These tests drive the guard directly, since a host has no
//! JNI environment.

use std::cell::RefCell;

use jni_sys::{jobject, JNIEnv};
use unlimitedphotos::jni::LocalRefs;

thread_local! {
    static DELETED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

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
