use std::fmt;

#[cfg(target_os = "android")]
pub fn log_args(args: fmt::Arguments) {
    use core::ffi::{c_char, c_int};
    use std::ffi::{CStr, CString};

    /// `ANDROID_LOG_DEBUG` from `<android/log.h>`.
    const ANDROID_LOG_DEBUG: c_int = 3;

    static TAG: &CStr = c"FGP/Native";
    static FORMAT: &CStr = c"%s";

    extern "C" {
        fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
    }

    // `__android_log_print` needs a NUL-terminated buffer, and the old code
    // printed raw bytes. Replacing NUL keeps the message instead of dropping
    // it, which matters when logging a malformed property value. The message is
    // formatted on the Rust side and passed as a `%s` argument, so a spoofed
    // property value can never be interpreted as a format string.
    let message = args.to_string().replace('\0', "\\0");
    let Ok(message) = CString::new(message) else {
        return;
    };
    unsafe {
        __android_log_print(
            ANDROID_LOG_DEBUG,
            TAG.as_ptr(),
            FORMAT.as_ptr(),
            message.as_ptr(),
        );
    }
}

/// Host builds (`cargo test`) have no logcat; the parsing code under test still
/// calls the same macro, so this keeps it compiling without changing behaviour.
#[cfg(not(target_os = "android"))]
pub fn log_args(_args: fmt::Arguments) {}
