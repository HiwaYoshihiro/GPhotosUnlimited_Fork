//! The Zygisk module entry point and the app-process side of the module.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;
use std::ffi::{CStr, CString};

use jni_sys::{jclass, jint, jobject, JNIEnv, JNI_OK};
use serde_json::{Map, Value};

use crate::companion::read_full;
use crate::config::{self, Settings};
use crate::filter;
use crate::hook;
use crate::jni;
use crate::limits::{MAX_CONFIG_SIZE, MAX_DEX_SIZE};
use crate::logd;
use crate::setfield;
use crate::zygisk::{
    self, ApiTable, AppSpecializeArgs, ModuleAbi, ServerSpecializeArgs, DLCLOSE_MODULE_LIBRARY,
    FORCE_DENYLIST_UNMOUNT, ZYGISK_API_VERSION,
};

/// The limits are `u64` because the companion compares them against file sizes;
/// the sizes on the wire are `c_long`, so compare in that type.
const MAX_DEX_SIZE_WIRE: libc::c_long = MAX_DEX_SIZE as libc::c_long;
const MAX_CONFIG_SIZE_WIRE: libc::c_long = MAX_CONFIG_SIZE as libc::c_long;

/// Process-wide mutable state.
///
/// Zygisk loads the library into the freshly forked child and then calls
/// `onLoad`, `preAppSpecialize` and `postAppSpecialize` on that child's single
/// thread; the C++ module relied on the same assumption via `static T module`.
/// A `Mutex` would work here too, but it would be held across JNI calls, and
/// these statics are re-initialised per process anyway.
struct Global<T>(UnsafeCell<T>);

// SAFETY: see the note on `Global`. Every access goes through `state_ptr()` /
// `api()`, which are only reachable from the Zygisk callbacks, and those are
// serialised by Zygisk on one thread.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    fn get(&self) -> *mut T {
        self.0.get()
    }
}

struct State {
    env: *mut JNIEnv,
    dex: Vec<u8>,
    json: Map<String, Value>,
    settings: Settings,
}

static STATE: Global<Option<State>> = Global::new(None);
static API: Global<*mut ApiTable> = Global::new(ptr::null_mut());

/// The module ABI, registered with Zygisk once per process.
///
/// A `static` rather than a `Box::into_raw`: Zygisk keeps this pointer for the
/// life of the process, so a heap allocation could never be reclaimed anyway,
/// and it was being made in every app process, including the ones that bail out
/// immediately. `impl_` is filled in with this static's own address.
static MODULE_ABI: Global<ModuleAbi> = Global::new(ModuleAbi {
    api_version: ZYGISK_API_VERSION,
    impl_: ptr::null_mut(),
    pre_app_specialize: Some(pre_app_specialize),
    post_app_specialize: Some(post_app_specialize),
    pre_server_specialize: Some(pre_server_specialize),
    // Not overridden by the module, but the ABI slot must still be a real
    // function: Zygisk calls it unconditionally for system_server.
    post_server_specialize: Some(post_server_specialize),
});

/// Raw pointer to the state, not `&mut`.
///
/// Handing out `&'static mut Option<State>` from a static would create a new
/// exclusive reference on every call, so holding one across a call to
/// `pre_app_specialize` (which takes another) would be aliasing UB even though
/// the process is single-threaded. Callers dereference it briefly instead.
fn state_ptr() -> *mut Option<State> {
    STATE.get()
}

unsafe fn api() -> *mut ApiTable {
    *API.get()
}

fn close_module() {
    unsafe { zygisk::set_option(api(), DLCLOSE_MODULE_LIBRARY) };
}

/// `REGISTER_ZYGISK_MODULE(GPhotosUnlimited)` plus the ABI trampolines.
#[no_mangle]
pub unsafe extern "C" fn zygisk_module_entry(table: *mut ApiTable, env: *mut JNIEnv) {
    if table.is_null() {
        return;
    }
    *API.get() = table;

    let abi = MODULE_ABI.get();
    // `impl` is only ever handed back to the callbacks below; Zygisk never
    // dereferences it, so pointing at our own state is enough.
    (*abi).impl_ = abi as *mut c_void;

    let Some(register) = (*table).register_module else {
        return;
    };
    if !register(table, abi) {
        return;
    }

    // SAFETY: Zygisk calls this on the child's single thread, and nothing else
    // writes `STATE` concurrently.
    (*state_ptr()) = Some(State {
        env,
        dex: Vec::new(),
        json: Map::new(),
        settings: Settings::default(),
    });
}

unsafe extern "C" fn pre_app_specialize(_impl: *mut c_void, args: *mut AppSpecializeArgs) {
    // Copy the env out rather than holding a reference into STATE across the
    // rest of this function: `inject` and the JNI calls below must not alias it.
    let Some(env) = (*state_ptr()).as_ref().map(|state| state.env) else {
        close_module();
        return;
    };
    if args.is_null() || env.is_null() {
        close_module();
        return;
    }
    let args = &*args;

    // In the C++ header these fields are `jstring &`, so `args->nice_name ==
    // nullptr` compared the jstring *value*. Here they are pointers to that
    // reference: the reference itself must be checked first, then the value.
    // GetStringUTFChars() aborts on a null jstring, so both checks have to
    // happen before the conversion rather than after it.
    if args.nice_name.is_null() || args.app_data_dir.is_null() {
        close_module();
        return;
    }
    let nice_name = *args.nice_name;
    let app_data_dir = *args.app_data_dir;
    if nice_name.is_null() || app_data_dir.is_null() {
        close_module();
        return;
    }

    let table = &**env;
    let (Some(get_utf_chars), Some(release_utf_chars)) =
        (table.GetStringUTFChars, table.ReleaseStringUTFChars)
    else {
        close_module();
        return;
    };

    let raw_process = get_utf_chars(env, nice_name, ptr::null_mut());
    // GetStringUTFChars raises OutOfMemoryError on failure. This runs in every
    // app process, so a pending exception left here would abort the *next* JNI
    // call the app makes, not just ours. Clear it before anything else, and do
    // not make the second JNI call while it is still pending.
    if jni::failed(env, "Couldn't read the process name") || raw_process.is_null() {
        if !raw_process.is_null() {
            release_utf_chars(env, nice_name, raw_process);
        }
        close_module();
        return;
    }

    let raw_dir = get_utf_chars(env, app_data_dir, ptr::null_mut());
    if jni::failed(env, "Couldn't read the app data directory") || raw_dir.is_null() {
        release_utf_chars(env, nice_name, raw_process);
        if !raw_dir.is_null() {
            release_utf_chars(env, app_data_dir, raw_dir);
        }
        close_module();
        return;
    }

    let process = CStr::from_ptr(raw_process).to_string_lossy().into_owned();
    let dir = CStr::from_ptr(raw_dir).to_string_lossy().into_owned();

    release_utf_chars(env, nice_name, raw_process);
    release_utf_chars(env, app_data_dir, raw_dir);

    // Both checks are exact, and both must pass: the main Google Photos
    // process in the Google Photos data directory. Anything else - another
    // app, a secondary Photos process, system_server - bails out here.
    let is_photos = filter::is_photos_data_dir(&dir);
    let is_photos_proc = filter::is_photos_main_process(&process);

    if !is_photos {
        close_module();
        return;
    }

    // We are in Google Photos now, force unmount.
    zygisk::set_option(api(), FORCE_DENYLIST_UNMOUNT);

    if !is_photos_proc {
        close_module();
        return;
    }

    let fd = zygisk::connect_companion(api());
    if fd < 0 {
        logd!("Couldn't connect to companion");
        close_module();
        return;
    }

    let long_size = core::mem::size_of::<libc::c_long>();
    let mut dex_size: libc::c_long = 0;
    let mut config_size: libc::c_long = 0;

    let sizes_ok = read_full(fd, &mut dex_size as *mut libc::c_long as *mut u8, long_size)
        && read_full(
            fd,
            &mut config_size as *mut libc::c_long as *mut u8,
            long_size,
        );
    if !sizes_ok {
        libc::close(fd);
        logd!("Couldn't read sizes from companion");
        close_module();
        return;
    }

    if dex_size < 1 {
        libc::close(fd);
        logd!("Couldn't read dex file");
        close_module();
        return;
    }
    if config_size < 1 {
        libc::close(fd);
        logd!("Couldn't read config file");
        close_module();
        return;
    }
    if dex_size > MAX_DEX_SIZE_WIRE || config_size > MAX_CONFIG_SIZE_WIRE {
        libc::close(fd);
        logd!(
            "Refusing oversized payload from companion: dex {} bytes, config {} bytes",
            dex_size,
            config_size
        );
        close_module();
        return;
    }

    logd!("Read from file descriptor for 'dex' -> {} bytes", dex_size);
    logd!(
        "Read from file descriptor for 'config' -> {} bytes",
        config_size
    );

    let mut dex = vec![0u8; dex_size as usize];
    let mut config_bytes = vec![0u8; config_size as usize];

    let payload_ok = read_full(fd, dex.as_mut_ptr(), dex.len())
        && read_full(fd, config_bytes.as_mut_ptr(), config_bytes.len());
    libc::close(fd);

    if !payload_ok {
        // postAppSpecialize() keys off an empty dex to skip a truncated dex.
        logd!("Couldn't read dex/config payload from companion");
        close_module();
        return;
    }

    let config_string = String::from_utf8_lossy(&config_bytes);
    let json = match config::parse_config(&config_string) {
        Value::Object(map) => map,
        // The old code aborted on a JSON document that was not an object;
        // treating it as "no config" leaves Photos working instead.
        _ => Map::new(),
    };

    // SAFETY: single-threaded; see `state_ptr`.
    let Some(state_ref) = (*state_ptr()).as_mut() else {
        return;
    };
    state_ref.dex = dex;
    state_ref.json = json;
}

unsafe extern "C" fn post_app_specialize(_impl: *mut c_void, _args: *const AppSpecializeArgs) {
    // Take the payload out of the static before doing anything with it. Holding
    // a reference into `STATE` across `inject()` (which re-enters JNI and the
    // hook) would alias it.
    let (env, dex, mut json, mut settings) = {
        let Some(state_ref) = (*state_ptr()).as_mut() else {
            return;
        };
        if state_ref.dex.is_empty() || state_ref.json.is_empty() {
            return;
        }
        // A truncated dex never reaches the loader: an empty payload is the
        // signal that the companion read failed.
        (
            state_ref.env,
            core::mem::take(&mut state_ref.dex),
            core::mem::take(&mut state_ref.json),
            state_ref.settings,
        )
    };

    let props = config::apply_settings(&mut json, &mut settings);
    if let Some(state_ref) = (*state_ptr()).as_mut() {
        state_ref.settings = settings;
    }

    if settings.spoof_props > 0 {
        hook::install(props, settings.verbose_logs);
    }

    // Each flag is checked on its own. Summing them let a negative value cancel
    // a positive one, so `spoofBuild=1` plus `spoofProvider=-1` skipped the
    // injection entirely.
    let spoof_java = settings.spoof_build > 0
        || settings.spoof_provider > 0
        || settings.spoof_signature > 0
        || settings.spoof_features > 0;
    if spoof_java {
        inject(env, &dex, &json, settings);
    }

    // `dex` and `json` are dropped here, freeing the buffers promptly as
    // `dexVector.clear()` / `json.clear()` did.
}

unsafe extern "C" fn pre_server_specialize(_impl: *mut c_void, _args: *mut ServerSpecializeArgs) {
    close_module();
}

unsafe extern "C" fn post_server_specialize(
    _impl: *mut c_void,
    _args: *const ServerSpecializeArgs,
) {
}

/// Loads the module dex and hands the remaining config to `EntryPoint`.
///
/// Every class and method lookup is null-checked. JNI treats a null `jmethodID`
/// passed to `Call*Method` as a fatal error that takes the process down, and the
/// C++ version only checked for a pending exception *after* making the call.
unsafe fn inject(env: *mut JNIEnv, dex: &[u8], json: &Map<String, Value>, settings: Settings) {
    if env.is_null() {
        return;
    }
    let table = &**env;

    let (
        Some(find_class),
        Some(get_static_method_id),
        Some(call_static_object),
        Some(get_method_id),
    ) = (
        table.FindClass,
        table.GetStaticMethodID,
        table.CallStaticObjectMethod,
        table.GetMethodID,
    )
    else {
        return;
    };

    // Every local reference created below is registered here, so the early
    // returns (there are many: a stale dex, a missing method, a pending
    // exception) release them too.
    let mut refs = jni::LocalRefs::new(env);

    logd!("JNI: Getting system classloader");
    let Some(cl_class) = find(env, find_class, c"java/lang/ClassLoader") else {
        return;
    };
    refs.push(cl_class as jobject);

    let Some(get_system_class_loader) = static_method(
        env,
        get_static_method_id,
        cl_class,
        c"getSystemClassLoader",
        c"()Ljava/lang/ClassLoader;",
    ) else {
        return;
    };
    let system_class_loader = call_static_object(env, cl_class, get_system_class_loader);
    if jni::failed(env, "Couldn't get system classloader") {
        return;
    }
    refs.push(system_class_loader);

    logd!("JNI: Creating module classloader");
    let (Some(new_object), Some(new_direct_byte_buffer)) =
        (table.NewObject, table.NewDirectByteBuffer)
    else {
        return;
    };

    let Some(dex_cl_class) = find(env, find_class, c"dalvik/system/InMemoryDexClassLoader") else {
        return;
    };
    refs.push(dex_cl_class as jobject);

    let Some(dex_cl_init) = method(
        env,
        get_method_id,
        dex_cl_class,
        c"<init>",
        c"(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
    ) else {
        return;
    };
    let buffer = new_direct_byte_buffer(env, dex.as_ptr() as *mut c_void, dex.len() as i64);
    refs.push(buffer);
    if jni::failed(env, "Couldn't create the dex buffer") {
        return;
    }

    let dex_cl = new_object(env, dex_cl_class, dex_cl_init, buffer, system_class_loader);
    refs.push(dex_cl);
    if jni::failed(env, "Couldn't create module classloader") {
        return;
    }
    // A null object without a pending exception would make the next call a
    // fatal error rather than a recoverable one.
    if dex_cl.is_null() {
        logd!("JNI: InMemoryDexClassLoader returned null");
        return;
    }

    logd!("JNI: Loading module class");
    let (Some(call_object_method), Some(new_string_utf), Some(register_natives)) = (
        table.CallObjectMethod,
        table.NewStringUTF,
        table.RegisterNatives,
    ) else {
        return;
    };

    let Some(load_class) = method(
        env,
        get_method_id,
        cl_class,
        c"loadClass",
        c"(Ljava/lang/String;)Ljava/lang/Class;",
    ) else {
        return;
    };
    let entry_class_name = new_string_utf(env, c"com.rev4n.unlimitedphotos.EntryPoint".as_ptr());
    refs.push(entry_class_name);
    if entry_class_name.is_null() {
        logd!("JNI: Couldn't create the EntryPoint class name");
        return;
    }
    let entry_class_obj = call_object_method(env, dex_cl, load_class, entry_class_name);
    refs.push(entry_class_obj);
    if jni::failed(env, "Couldn't load module class") {
        return;
    }
    let entry_class = entry_class_obj as jclass;
    if entry_class.is_null() {
        logd!("JNI: EntryPoint class was not found");
        return;
    }

    let methods = [setfield::native_method()];
    if register_natives(env, entry_class, methods.as_ptr(), methods.len() as jint) != JNI_OK {
        jni::failed(env, "Couldn't register setFieldNative");
        return;
    }

    logd!("JNI: Sending JSON");
    let Some(receive_json) = static_method(
        env,
        get_static_method_id,
        entry_class,
        c"receiveJson",
        c"(Ljava/lang/String;)V",
    ) else {
        return;
    };

    // `serde_json` serialises map keys in sorted order, matching what
    // `nlohmann::json::dump()` produced for the Java side.
    let json_text = serde_json::to_string(json).unwrap_or_else(|_| "{}".to_string());
    let Ok(json_c) = CString::new(json_text) else {
        return;
    };
    let java_str = new_string_utf(env, json_c.as_ptr());
    refs.push(java_str);
    if java_str.is_null() {
        logd!("JNI: Couldn't create the config string");
        return;
    }
    if let Some(call_static_void) = table.CallStaticVoidMethod {
        call_static_void(env, entry_class, receive_json, java_str);
    }
    if jni::failed(env, "EntryPoint.receiveJson threw") {
        return;
    }

    logd!("JNI: Calling EntryPoint.init");
    let Some(entry_init) =
        static_method(env, get_static_method_id, entry_class, c"init", c"(IIIII)V")
    else {
        return;
    };
    if let Some(call_static_void) = table.CallStaticVoidMethod {
        call_static_void(
            env,
            entry_class,
            entry_init,
            settings.verbose_logs as jint,
            settings.spoof_build as jint,
            settings.spoof_provider as jint,
            settings.spoof_signature as jint,
            settings.spoof_features as jint,
        );
    }
    jni::failed(env, "EntryPoint.init threw");
}

type FindClass = unsafe extern "system" fn(*mut JNIEnv, *const core::ffi::c_char) -> jclass;
type GetMethodId = unsafe extern "system" fn(
    *mut JNIEnv,
    jclass,
    *const core::ffi::c_char,
    *const core::ffi::c_char,
) -> jni_sys::jmethodID;
type GetStaticMethodId = GetMethodId;

/// `FindClass` that reports a failure instead of leaving a null `jclass` behind.
unsafe fn find(env: *mut JNIEnv, find_class: FindClass, name: &CStr) -> Option<jclass> {
    let class = find_class(env, name.as_ptr());
    if class.is_null() {
        jni::failed(env, "FindClass returned null");
        return None;
    }
    Some(class)
}

unsafe fn method(
    env: *mut JNIEnv,
    get_method_id: GetMethodId,
    class: jclass,
    name: &CStr,
    signature: &CStr,
) -> Option<jni_sys::jmethodID> {
    let method = get_method_id(env, class, name.as_ptr(), signature.as_ptr());
    if method.is_null() {
        // GetMethodID raises NoSuchMethodError, which `failed` clears and logs.
        jni::failed(env, "GetMethodID returned null");
        return None;
    }
    Some(method)
}

unsafe fn static_method(
    env: *mut JNIEnv,
    get_static_method_id: GetStaticMethodId,
    class: jclass,
    name: &CStr,
    signature: &CStr,
) -> Option<jni_sys::jmethodID> {
    let method = get_static_method_id(env, class, name.as_ptr(), signature.as_ptr());
    if method.is_null() {
        jni::failed(env, "GetStaticMethodID returned null");
        return None;
    }
    Some(method)
}
