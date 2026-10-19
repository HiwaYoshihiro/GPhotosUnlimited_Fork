//! Hand-written bindings for `zygisk.hpp` (ZYGISK_API_VERSION 2).
//!
//! The layout here is ABI, not style: `api_version` is `long` (8 bytes on
//! arm64, 4 on armeabi-v7a), the `*SpecializeArgs` fields are references in
//! C++ and therefore pointers here, and `module_abi.impl` is only ever passed
//! back to the callbacks without being dereferenced.

use core::ffi::{c_char, c_int, c_long, c_void};

use jni_sys::{jboolean, jint, jintArray, jlong, jobjectArray, jstring, JNIEnv, JNINativeMethod};

pub const ZYGISK_API_VERSION: c_long = 2;

/// `enum Option : int`
pub const FORCE_DENYLIST_UNMOUNT: c_int = 0;
pub const DLCLOSE_MODULE_LIBRARY: c_int = 1;

/// The opaque `prop_info` the platform passes to the property hook.
pub type PropInfo = c_void;

#[repr(C)]
pub struct ApiTable {
    pub impl_: *mut c_void,
    pub register_module: Option<unsafe extern "C" fn(*mut ApiTable, *mut ModuleAbi) -> bool>,
    pub hook_jni_native_methods:
        Option<unsafe extern "C" fn(*mut JNIEnv, *const c_char, *mut JNINativeMethod, c_int)>,
    pub plt_hook_register:
        Option<unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void, *mut *mut c_void)>,
    pub plt_hook_exclude: Option<unsafe extern "C" fn(*const c_char, *const c_char)>,
    pub plt_hook_commit: Option<unsafe extern "C" fn() -> bool>,
    pub connect_companion: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    pub set_option: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
    pub get_module_dir: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    pub get_flags: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
}

#[repr(C)]
pub struct ModuleAbi {
    pub api_version: c_long,
    pub impl_: *mut c_void,
    pub pre_app_specialize: Option<unsafe extern "C" fn(*mut c_void, *mut AppSpecializeArgs)>,
    pub post_app_specialize: Option<unsafe extern "C" fn(*mut c_void, *const AppSpecializeArgs)>,
    pub pre_server_specialize: Option<unsafe extern "C" fn(*mut c_void, *mut ServerSpecializeArgs)>,
    pub post_server_specialize:
        Option<unsafe extern "C" fn(*mut c_void, *const ServerSpecializeArgs)>,
}

#[repr(C)]
pub struct AppSpecializeArgs {
    pub uid: *mut jint,
    pub gid: *mut jint,
    pub gids: *mut jintArray,
    pub runtime_flags: *mut jint,
    pub mount_external: *mut jint,
    pub se_info: *mut jstring,
    pub nice_name: *mut jstring,
    pub instruction_set: *mut jstring,
    pub app_data_dir: *mut jstring,
    // Optional arguments; the pointers may be null.
    pub is_child_zygote: *const jboolean,
    pub is_top_app: *const jboolean,
    pub pkg_data_info_list: *const jobjectArray,
    pub whitelisted_data_info_list: *const jobjectArray,
    pub mount_data_dirs: *const jboolean,
    pub mount_storage_dirs: *const jboolean,
}

#[repr(C)]
pub struct ServerSpecializeArgs {
    pub uid: *mut jint,
    pub gid: *mut jint,
    pub gids: *mut jintArray,
    pub runtime_flags: *mut jint,
    pub permitted_capabilities: *mut jlong,
    pub effective_capabilities: *mut jlong,
}

/// Sets a Zygisk option (`DLCLOSE_MODULE_LIBRARY`, `FORCE_DENYLIST_UNMOUNT`, ...).
///
/// # Safety
/// `api` must be the table Zygisk passed to `zygisk_module_entry`, and this may
/// only be called from the specialization callbacks.
pub unsafe fn set_option(api: *mut ApiTable, option: c_int) {
    if api.is_null() {
        return;
    }
    if let Some(set_option) = (*api).set_option {
        set_option((*api).impl_, option);
    }
}

/// Opens a socket to the root companion, or returns -1.
///
/// # Safety
/// Same contract as [`set_option`]: only valid inside `preAppSpecialize`.
pub unsafe fn connect_companion(api: *mut ApiTable) -> c_int {
    if api.is_null() {
        return -1;
    }
    match (*api).connect_companion {
        Some(connect) => connect((*api).impl_),
        None => -1,
    }
}
