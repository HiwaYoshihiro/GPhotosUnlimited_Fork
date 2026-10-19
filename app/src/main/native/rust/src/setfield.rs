//! `EntryPoint.setFieldNative`, registered into the injected dex.

use core::ffi::{c_char, c_void};
use core::ptr;

use jni_sys::{jboolean, jclass, jint, jlong, jobject, jstring, JNIEnv};

/// Signature of the Java method, kept byte-identical to the Java declaration.
pub const SIGNATURE: &[u8] =
    b"(Ljava/lang/Class;Ljava/lang/reflect/Field;Ljava/lang/String;Ljava/lang/Object;)V\0";

/// Which `SetStatic*Field` call a `Build` field needs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Object,
    Int,
    Long,
    Boolean,
}

/// Sets a static field on a `Build`-like class.
///
/// The JNI table is reached through raw pointers because the field and its
/// type come from Java reflection; a missing table entry is treated as a
/// no-op instead of being assumed present.
pub unsafe extern "C" fn set_field_native(
    env: *mut JNIEnv,
    _clazz: jclass,
    target_class: jclass,
    field_obj: jobject,
    type_obj: jstring,
    value_obj: jobject,
) {
    if env.is_null() || target_class.is_null() || field_obj.is_null() || type_obj.is_null() {
        return;
    }
    let table = &**env;

    let (Some(from_reflected_field), Some(get_utf_chars), Some(release_utf_chars)) = (
        table.FromReflectedField,
        table.GetStringUTFChars,
        table.ReleaseStringUTFChars,
    ) else {
        return;
    };

    let field_id = from_reflected_field(env, field_obj);
    if field_id.is_null() {
        return;
    }

    let type_name = get_utf_chars(env, type_obj, ptr::null_mut());
    if type_name.is_null() {
        return;
    }

    // The type string is a short ASCII constant; compare it by value.
    let field_type = match core::ffi::CStr::from_ptr(type_name).to_str() {
        Ok("java.lang.String") => Some(FieldType::Object),
        Ok("int") => Some(FieldType::Int),
        Ok("long") => Some(FieldType::Long),
        Ok("boolean") => Some(FieldType::Boolean),
        _ => None,
    };

    match field_type {
        Some(FieldType::Object) => {
            if let Some(set) = table.SetStaticObjectField {
                set(env, target_class, field_id, value_obj);
            }
        }
        Some(FieldType::Int) => {
            if let Some(value) = unbox(env, value_obj, Boxed::Int) {
                if let Some(set) = table.SetStaticIntField {
                    set(env, target_class, field_id, value as jint);
                }
            }
        }
        Some(FieldType::Long) => {
            if let Some(value) = unbox(env, value_obj, Boxed::Long) {
                if let Some(set) = table.SetStaticLongField {
                    set(env, target_class, field_id, value as jlong);
                }
            }
        }
        Some(FieldType::Boolean) => {
            if let Some(value) = unbox(env, value_obj, Boxed::Boolean) {
                if let Some(set) = table.SetStaticBooleanField {
                    set(env, target_class, field_id, value as jboolean);
                }
            }
        }
        None => {}
    }

    release_utf_chars(env, type_obj, type_name);
}

enum Boxed {
    Int,
    Long,
    Boolean,
}

/// Unboxes a `java.lang.Integer` / `Long` / `Boolean`.
unsafe fn unbox(env: *mut JNIEnv, value_obj: jobject, kind: Boxed) -> Option<i64> {
    let table = &**env;
    let (find_class, get_method_id) = (table.FindClass?, table.GetMethodID?);

    let (class_name, method_name, method_sig): (&[u8], &[u8], &[u8]) = match kind {
        Boxed::Int => (b"java/lang/Integer\0", b"intValue\0", b"()I\0"),
        Boxed::Long => (b"java/lang/Long\0", b"longValue\0", b"()J\0"),
        Boxed::Boolean => (b"java/lang/Boolean\0", b"booleanValue\0", b"()Z\0"),
    };

    let class = find_class(env, class_name.as_ptr() as *const c_char);
    if class.is_null() {
        return None;
    }
    let method = get_method_id(
        env,
        class,
        method_name.as_ptr() as *const c_char,
        method_sig.as_ptr() as *const c_char,
    );
    if method.is_null() {
        if let Some(delete) = table.DeleteLocalRef {
            delete(env, class);
        }
        return None;
    }

    let value = match kind {
        Boxed::Int => table
            .CallIntMethod
            .map(|call| i64::from(call(env, value_obj, method))),
        Boxed::Long => table
            .CallLongMethod
            .map(|call| call(env, value_obj, method)),
        Boxed::Boolean => table
            .CallBooleanMethod
            .map(|call| i64::from(call(env, value_obj, method) != 0)),
    };

    if let Some(delete) = table.DeleteLocalRef {
        delete(env, class);
    }
    value
}

/// `JNINativeMethod` for `setFieldNative`.
pub fn native_method() -> jni_sys::JNINativeMethod {
    // SAFETY: both pointers reference static byte strings with a trailing NUL,
    // so the table entry stays valid for the life of the process.
    jni_sys::JNINativeMethod {
        name: c"setFieldNative".as_ptr() as *mut c_char,
        signature: SIGNATURE.as_ptr() as *mut c_char,
        fnPtr: set_field_native as *mut c_void,
    }
}
