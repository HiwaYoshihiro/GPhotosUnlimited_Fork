//! The `__system_property_read_callback` hook.
//!
//! Replaces the `std::map` + shadowhook code from `main.cpp`. The property
//! table is handed over once, before the hook goes live, and is only read
//! afterwards, so the `const char *` values handed to the platform point into
//! storage that lives as long as the process does. Returning a pointer to a
//! temporary here would be a use-after-free inside the property callback.

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicI32, Ordering};
use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString};
use std::sync::{Mutex, OnceLock};

use crate::config::Props;
use crate::logd;
use crate::zygisk::PropInfo;

/// `void (*)(void *, const char *, const char *, uint32_t)`
pub type PropertyCallback = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, u32);

/// `void (*)(const prop_info *, void (*)(...), void *)`
type ReadCallback = unsafe extern "C" fn(*const PropInfo, Option<PropertyCallback>, *mut c_void);

/// `SHADOWHOOK_MODE_UNIQUE`
const SHADOWHOOK_MODE_UNIQUE: c_int = 1;

extern "C" {
    fn shadowhook_init(mode: c_int, debuggable: bool) -> c_int;
    fn shadowhook_hook_sym_addr(
        sym_addr: *mut c_void,
        new_addr: *mut c_void,
        orig_addr: *mut *mut c_void,
    ) -> *mut c_void;
    fn shadowhook_get_errno() -> c_int;
}

/// The original `__system_property_read_callback`, filled in by shadowhook.
static mut ORIGINAL: *mut c_void = ptr::null_mut();

/// Property table, immutable once installed.
static PROPS: OnceLock<BTreeMap<String, CString>> = OnceLock::new();
static VERBOSE: AtomicI32 = AtomicI32::new(0);

/// Callback registered per cookie.
///
/// The old code used an unsynchronised `std::map` here, written from every
/// thread that reads a property. A `Mutex` fixes that, and the lock is never
/// held while the callback runs, so a callback that itself reads a property
/// cannot deadlock.
static CALLBACKS: Mutex<Option<HashMap<usize, PropertyCallback>>> = Mutex::new(None);

/// Installs the hook. `props` is consumed so its `CString` buffers stay put.
pub unsafe fn install(props: Props, verbose_logs: i32) {
    VERBOSE.store(verbose_logs, Ordering::Relaxed);
    let _ = PROPS.set(
        props
            .into_iter()
            // An interior NUL cannot be represented in a `CString`. `c_str()`
            // in the old code would hand the platform a buffer that stops at
            // that NUL anyway, so truncating keeps the same visible value
            // instead of dropping the override entirely.
            .map(|(name, value)| {
                let value = match value.split_once('\0') {
                    Some((head, _)) => head.to_string(),
                    None => value,
                };
                (name, CString::new(value).unwrap_or_default())
            })
            .collect(),
    );

    shadowhook_init(SHADOWHOOK_MODE_UNIQUE, false);

    // `RTLD_DEFAULT` is `(void *)0` on bionic.
    let sym_addr = libc::dlsym(ptr::null_mut(), c"__system_property_read_callback".as_ptr());
    if sym_addr.is_null() {
        logd!("Couldn't find '__system_property_read_callback' via dlsym");
        return;
    }

    let handle = shadowhook_hook_sym_addr(
        sym_addr,
        my_system_property_read_callback as *mut c_void,
        ptr::addr_of_mut!(ORIGINAL),
    );

    if handle.is_null() {
        logd!(
            "Couldn't hook '__system_property_read_callback'. Error: {}",
            shadowhook_get_errno()
        );
        return;
    }
    logd!(
        "Found '__system_property_read_callback' handle at {:p}",
        handle
    );
}

fn original() -> Option<ReadCallback> {
    // Safe: shadowhook writes this once, before the hook can be reached.
    let raw = unsafe { ptr::read(ptr::addr_of!(ORIGINAL)) };
    if raw.is_null() {
        None
    } else {
        Some(unsafe { core::mem::transmute::<*mut c_void, ReadCallback>(raw) })
    }
}

unsafe extern "C" fn my_system_property_read_callback(
    pi: *const PropInfo,
    callback: Option<PropertyCallback>,
    cookie: *mut c_void,
) {
    let Some(original) = original() else {
        return;
    };

    // Null arguments go straight through, exactly as before: the platform is
    // allowed to ask for the raw callback without a cookie.
    match callback {
        Some(callback) if !pi.is_null() && !cookie.is_null() => {
            register(cookie, callback);
            original(pi, Some(modify_callback), cookie);
        }
        _ => original(pi, callback, cookie),
    }
}

fn register(cookie: *mut c_void, callback: PropertyCallback) {
    let mut guard = CALLBACKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .get_or_insert_with(HashMap::new)
        .insert(cookie as usize, callback);
}

fn lookup(cookie: *mut c_void) -> Option<PropertyCallback> {
    let guard = CALLBACKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.as_ref()?.get(&(cookie as usize)).copied()
}

unsafe extern "C" fn modify_callback(
    cookie: *mut c_void,
    name: *const c_char,
    value: *const c_char,
    serial: u32,
) {
    if cookie.is_null() || name.is_null() || value.is_null() {
        return;
    }
    let Some(callback) = lookup(cookie) else {
        return;
    };

    let old_value = value;
    let mut new_value = value;
    let prop = CStr::from_ptr(name).to_string_lossy();

    if let Some(props) = PROPS.get() {
        if let Some(overridden) = exact(props, &prop).or_else(|| wildcard(props, &prop)) {
            new_value = overridden;
        }
    }

    // Pointer identity, as in the original: "unchanged" means no override was
    // applied at all.
    if old_value == new_value {
        if VERBOSE.load(Ordering::Relaxed) > 99 {
            logd!("[{}]: {} (unchanged)", prop, string(old_value));
        }
    } else {
        logd!("[{}]: {} -> {}", prop, string(old_value), string(new_value));
    }

    callback(cookie, name, new_value, serial);
}

fn exact(props: &BTreeMap<String, CString>, name: &str) -> Option<*const c_char> {
    props.get(name).map(|value| value.as_ptr())
}

/// Leading `*` wildcard match. `BTreeMap` keeps the same sorted order
/// `std::map` had, so the first matching wildcard is still the winner.
fn wildcard(props: &BTreeMap<String, CString>, name: &str) -> Option<*const c_char> {
    props
        .iter()
        .find(|(key, _)| match key.strip_prefix('*') {
            Some(suffix) => name.ends_with(suffix),
            None => false,
        })
        .map(|(_, value)| value.as_ptr())
}

fn string(value: *const c_char) -> String {
    if value.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}
