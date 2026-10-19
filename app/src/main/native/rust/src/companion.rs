//! The root companion.
//!
//! Runs in a separate superuser daemon (one per ABI) and hands the module dex
//! plus the user config to the app process over a Unix socket. Concurrent
//! invocation is expected; nothing here is shared between connections.

use core::ffi::{c_int, c_long};
use std::mem::size_of;

use crate::file_io::read_capped;
use crate::limits::{MAX_CONFIG_SIZE, MAX_DEX_SIZE};

const DEX_FILE_PATH: &str = "/data/adb/modules/unlimitedphotos/classes.dex";
const PROP_FILE_PATH: &str = "/data/adb/modules/unlimitedphotos/fgp.prop";
const CUSTOM_PROP_FILE_PATH: &str = "/data/adb/modules/unlimitedphotos/custom.fgp.prop";
const CUSTOM_JSON_FILE_PATH: &str = "/data/adb/modules/unlimitedphotos/custom.fgp.json";

/// `REGISTER_ZYGISK_COMPANION(companion)`
#[no_mangle]
pub unsafe extern "C" fn zygisk_companion_entry(client: c_int) {
    let dex = read_file(DEX_FILE_PATH, MAX_DEX_SIZE);
    let config = read_config();

    let dex_size = dex.len() as c_long;
    let config_size = config.len() as c_long;

    let written = write_full(
        client,
        &dex_size as *const c_long as *const u8,
        size_of::<c_long>(),
    ) && write_full(
        client,
        &config_size as *const c_long as *const u8,
        size_of::<c_long>(),
    ) && write_full(client, dex.as_ptr(), dex.len())
        && write_full(client, config.as_ptr(), config.len());

    if !written {
        crate::logd!("Couldn't write dex/config to the module");
    }
}

/// User config, most specific first: `custom.fgp.prop`, then
/// `custom.fgp.json`, then the bundled `fgp.prop`.
fn read_config() -> Vec<u8> {
    for path in [CUSTOM_PROP_FILE_PATH, CUSTOM_JSON_FILE_PATH, PROP_FILE_PATH] {
        if let Some(bytes) = read_capped(path, MAX_CONFIG_SIZE) {
            return bytes;
        }
    }
    Vec::new()
}

/// Reads a whole file. A missing or unreadable file yields an empty buffer,
/// which the app side rejects before touching the dex loader.
fn read_file(path: &str, limit: u64) -> Vec<u8> {
    read_capped(path, limit).unwrap_or_default()
}

/// One `read()`/`write()` can come up short on a socket, which would leave a
/// truncated dex or config behind, so keep going until the whole payload has
/// moved.
unsafe fn write_full(fd: c_int, mut buffer: *const u8, mut size: usize) -> bool {
    while size > 0 {
        let moved = libc::write(fd, buffer as *const libc::c_void, size);
        if moved <= 0 {
            if moved < 0
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            {
                continue;
            }
            return false;
        }
        buffer = buffer.add(moved as usize);
        size -= moved as usize;
    }
    true
}

/// Same as `write_full`, for the app side reading from the companion.
pub unsafe fn read_full(fd: c_int, mut buffer: *mut u8, mut size: usize) -> bool {
    while size > 0 {
        let moved = libc::read(fd, buffer as *mut libc::c_void, size);
        if moved <= 0 {
            if moved < 0
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            {
                continue;
            }
            return false;
        }
        buffer = buffer.add(moved as usize);
        size -= moved as usize;
    }
    true
}
