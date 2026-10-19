use jni_sys::JNIEnv;

use crate::logd;

/// Clears a pending exception so the next JNI transition does not abort.
///
/// Leaving an exception pending (or calling through a null method ID, which is
/// a fatal error) would take Google Photos down on the next JNI call. This is
/// what a stale `classes.dex` from a partially applied module update looks
/// like from here, so bail out instead of continuing.
pub unsafe fn failed(env: *mut JNIEnv, what: &str) -> bool {
    if env.is_null() {
        return true;
    }
    let table = &**env;
    let Some(check) = table.ExceptionCheck else {
        return false;
    };
    if check(env) == 0 {
        return false;
    }
    if let Some(clear) = table.ExceptionClear {
        clear(env);
    }
    logd!("JNI: {}", what);
    true
}
