//! Native half of the Google Photos Unlimited module.
//!
//! This is a 1:1 port of the former `main.cpp`. The exported ABI is unchanged:
//! `zygisk_module_entry` and `zygisk_companion_entry` are still what Zygisk
//! looks up, and `EntryPoint.setFieldNative` is still registered with the same
//! name and signature.

pub mod config;
pub mod file_io;
pub mod filter;
pub mod jni;
pub mod limits;
pub mod props;
pub mod zygisk;

mod log;

// Everything below is Android-only: it calls bionic, JNI and shadowhook. The
// modules are gated so `cargo test` can still exercise the pure logic (config
// parsing, property dispatch, ABI layout) on a host machine.
#[cfg(target_os = "android")]
mod companion;
#[cfg(target_os = "android")]
mod hook;
#[cfg(target_os = "android")]
mod module;
#[cfg(target_os = "android")]
mod setfield;

/// Logs to logcat under the `FGP/Native` tag, matching the old `LOGD` macro.
#[macro_export]
macro_rules! logd {
    ($($arg:tt)*) => {
        $crate::log::log_args(format_args!($($arg)*))
    };
}

/// Stops shadowhook from hooking the linker's `soinfo` ctor/dtor walk.
///
/// `shadowhook.c` calls this symbol during `shadowhook_init()`. The upstream
/// implementation hooks `call_constructors`/`call_destructors` inside the
/// linker, which is both unnecessary for us and an extra thing to be
/// fingerprinted, so the CMake build renames the real one
/// (`sh_linker_init_dummy`, via a compile definition) and this stub returns
/// success. Keep it: without it `shadowhook_init` fails with
/// `SHADOWHOOK_ERRNO_INIT_LINKER` and system property spoofing silently stops
/// working.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn sh_linker_init() -> core::ffi::c_int {
    0
}
