//! The property table and the callback that rewrites values.
//!
//! Kept separate from `hook.rs` (which is Android-only) so the dispatch can be
//! tested on a host: the interesting failures here are lifetime and routing
//! bugs, not shadowhook itself.

use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicI32, Ordering};
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::sync::OnceLock;

use crate::config::Props;
use crate::logd;

/// `void (*)(void *, const char *, const char *, uint32_t)`
pub type PropertyCallback = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, u32);

/// One `__system_property_read_callback` invocation.
///
/// The platform calls the callback synchronously, before
/// `__system_property_read_callback` returns, so this can live on the caller's
/// stack. That is what makes the old global `cookie -> callback` map
/// unnecessary: there is no shared state to grow, and a reused cookie can no
/// longer resolve to a callback left over from an earlier call.
#[repr(C)]
pub struct CallContext {
    callback: PropertyCallback,
    cookie: *mut c_void,
}

impl CallContext {
    pub fn new(callback: PropertyCallback, cookie: *mut c_void) -> Self {
        Self { callback, cookie }
    }

    /// The context as the opaque cookie handed to the real implementation.
    pub fn as_void_ptr(&mut self) -> *mut c_void {
        self as *mut Self as *mut c_void
    }
}

/// Property table, immutable once installed.
///
/// Values are `CString` so the `const char *` handed to the platform points
/// into storage that lives as long as the process does.
static PROPS: OnceLock<BTreeMap<String, CString>> = OnceLock::new();
static VERBOSE: AtomicI32 = AtomicI32::new(0);

pub fn set_verbose(level: i32) {
    VERBOSE.store(level, Ordering::Relaxed);
}

/// Installs the table. A second call is ignored: the hook is installed once per
/// process, and replacing the map would invalidate pointers already handed out.
pub fn install_props(props: Props) {
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
}

/// The override for `name`, exact match first, then the first wildcard match.
pub fn resolve(props: &BTreeMap<String, CString>, name: &str) -> Option<*const c_char> {
    props
        .get(name)
        .map(|value| value.as_ptr())
        .or_else(|| wildcard(props, name))
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

/// The callback installed in place of the caller's.
///
/// `context` is the [`CallContext`] from the matching
/// `__system_property_read_callback` call, still on that call's stack.
///
/// # Safety
/// `context` must point to a live [`CallContext`] for the duration of the call,
/// and `name`/`value` must be the NUL-terminated strings the platform passed.
/// This is only ever installed as a property callback, which the platform calls
/// synchronously.
pub unsafe extern "C" fn modify_callback(
    context: *mut c_void,
    name: *const c_char,
    value: *const c_char,
    serial: u32,
) {
    let Some(context) = (context as *const CallContext).as_ref() else {
        return;
    };
    if name.is_null() || value.is_null() {
        return;
    }

    let prop = CStr::from_ptr(name).to_string_lossy();
    let new_value = PROPS
        .get()
        .and_then(|props| resolve(props, &prop))
        .unwrap_or(value);

    // Pointer identity, as in the original: "unchanged" means no override was
    // applied at all. Both branches are behind `verboseLogs`; this runs on
    // every property read in the process, so it must not log by default.
    let verbose = VERBOSE.load(Ordering::Relaxed);
    if new_value == value {
        if verbose > 99 {
            logd!("[{}]: {} (unchanged)", prop, string(value));
        }
    } else if verbose > 0 {
        logd!("[{}]: {} -> {}", prop, string(value), string(new_value));
    }

    (context.callback)(context.cookie, name, new_value, serial);
}

fn string(value: *const c_char) -> String {
    if value.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}
