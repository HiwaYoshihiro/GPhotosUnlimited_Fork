//! The `__system_property_read_callback` hook.
//!
//! Only the shadowhook plumbing lives here; the property table and the
//! callback that rewrites values are in `props.rs` so they can be tested
//! without a device.

use core::ffi::{c_int, c_void};
use core::ptr;

use crate::config::Props;
use crate::logd;
use crate::props::{self, CallContext, PropertyCallback};
use crate::zygisk::PropInfo;

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

/// Installs the hook. `props` is consumed so its `CString` buffers stay put.
pub unsafe fn install(props: Props, verbose_logs: i32) {
    props::set_verbose(verbose_logs);
    props::install_props(props);

    let errno = shadowhook_init(SHADOWHOOK_MODE_UNIQUE, false);
    if errno != 0 {
        logd!("Couldn't initialize shadowhook. Error: {}", errno);
        return;
    }

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
        Some(callback) if !pi.is_null() => {
            // The real implementation calls `callback` before returning, so a
            // stack context is safe here. Passing the caller's cookie through
            // unchanged would force us back to a global map to find the
            // callback again, and that map was the leak.
            let mut context = CallContext::new(callback, cookie);
            original(
                pi,
                Some(props::modify_callback),
                context.as_void_ptr() as *mut c_void,
            );
        }
        _ => original(pi, callback, cookie),
    }
}
