use jni_sys::{jchar, jobject, jsize, jstring, JNIEnv};

use crate::logd;

/// Clears a pending exception so the next JNI transition does not abort.
///
/// Leaving an exception pending (or calling through a null method ID, which is
/// a fatal error) would take Google Photos down on the next JNI call. This is
/// what a stale `classes.dex` from a partially applied module update looks
/// like from here, so bail out instead of continuing.
///
/// # Safety
/// `env` must be a valid `JNIEnv` for the current thread, or null.
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

/// Creates a `java.lang.String` from Rust text.
///
/// `NewStringUTF` takes *modified* UTF-8, where a character outside the BMP is
/// written as two three-byte surrogate escapes. A Rust string is plain UTF-8,
/// which uses one four-byte sequence instead, so handing it to `NewStringUTF`
/// is outside the JNI contract (CheckJNI rejects it). `NewString` takes UTF-16,
/// which is exactly what `encode_utf16` produces, so this is correct for every
/// input and does not need a NUL terminator.
///
/// Returns `None` on failure with the pending exception already cleared:
/// leaving it set would abort the next JNI call made on this thread after
/// `inject` returns to Zygisk.
///
/// # Safety
/// `env` must be a valid `JNIEnv` for the current thread, or null.
pub unsafe fn new_string(env: *mut JNIEnv, text: &str) -> Option<jstring> {
    if env.is_null() {
        return None;
    }
    let new_string = (&**env).NewString?;
    let utf16: Vec<jchar> = text.encode_utf16().collect();
    let Ok(len) = jsize::try_from(utf16.len()) else {
        return None;
    };
    let string = new_string(env, utf16.as_ptr(), len);
    if string.is_null() {
        failed(env, "Couldn't create a Java string");
        return None;
    }
    Some(string)
}

type DeleteLocalRef = unsafe extern "system" fn(*mut JNIEnv, jobject);

/// Local references owned by one JNI call, deleted when dropped.
///
/// `inject` used to delete its references only on the success path, so every
/// early return leaked whatever had been created so far. JNI only reserves room
/// for a small number of local references per native frame, and the process is
/// long-lived, so this needs to be correct on every exit rather than on the
/// happy path only.
pub struct LocalRefs {
    env: *mut JNIEnv,
    delete: Option<DeleteLocalRef>,
    refs: Vec<jobject>,
}

impl LocalRefs {
    /// Collects references created against `env`, using its own `DeleteLocalRef`.
    ///
    /// # Safety
    /// `env` must be a valid `JNIEnv` for the current thread.
    pub unsafe fn new(env: *mut JNIEnv) -> Self {
        let delete = if env.is_null() {
            None
        } else {
            (&**env).DeleteLocalRef
        };
        Self::with_deleter(env, delete)
    }

    /// Same as [`LocalRefs::new`], with an explicit deleter.
    ///
    /// Exposed so the cleanup can be exercised on a host, where there is no JNI
    /// environment to take a table from.
    pub fn with_deleter(env: *mut JNIEnv, delete: Option<DeleteLocalRef>) -> Self {
        Self {
            env,
            delete,
            refs: Vec::new(),
        }
    }

    /// Takes ownership of `obj`, ignoring null references.
    pub fn push(&mut self, obj: jobject) {
        if !obj.is_null() {
            self.refs.push(obj);
        }
    }
}

impl Drop for LocalRefs {
    fn drop(&mut self) {
        let Some(delete) = self.delete else {
            return;
        };
        for reference in self.refs.drain(..) {
            // SAFETY: every reference was created on this thread and is still
            // alive, and `env` outlives the guard (it is the caller's frame).
            unsafe { delete(self.env, reference) };
        }
    }
}
