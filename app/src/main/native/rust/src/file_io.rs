//! Bounded file reads.
//!
//! The companion reads the module dex and the user config from a root-owned
//! directory and then ships them to the app process. Both files have a hard
//! size limit ([`crate::limits`]), and the limit has to hold even if the file
//! changes while it is being read: a metadata check followed by an unbounded
//! read leaves a window where a growing or replaced file is pulled into memory
//! in full.

use std::fs::File;
use std::io::{ErrorKind, Read};

use crate::logd;

/// Reads `path` only if it is a regular file no larger than `limit`.
///
/// The handle is opened once and both the size hint and the read come from it,
/// so replacing the path mid-read cannot redirect the read. `take(limit + 1)`
/// is the hard cap: it lets the read notice one byte past the limit without
/// ever moving more than that, which is what makes a file that grows during the
/// read harmless. Checking the length after an unbounded read would be too
/// late, since the allocation has already happened.
///
/// A missing or unreadable file yields `None`; callers decide whether that is
/// fatal.
pub fn read_capped(path: &str, limit: u64) -> Option<Vec<u8>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            logd!("Couldn't open '{}': {}", path, error);
            return None;
        }
    };

    // Metadata from the open handle, not from the path: the file behind the
    // descriptor cannot be swapped for another one.
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        logd!("Refusing '{}': not a regular file", path);
        return None;
    }
    if metadata.len() > limit {
        logd!(
            "Refusing '{}': {} bytes, limit {}",
            path,
            metadata.len(),
            limit
        );
        return None;
    }

    // One byte past the limit distinguishes "exactly at the limit" from "over
    // it" without trusting the metadata for the size actually read.
    let cap = limit.saturating_add(1);
    // The metadata is only a capacity hint. A file that grows past it makes the
    // vector grow, which `take` still bounds.
    let mut bytes = Vec::with_capacity(metadata.len().min(cap) as usize);
    let mut reader = file.take(cap);
    // Read in fixed chunks rather than `read_to_end`: `Take::read_to_end` may
    // reserve the whole limit up front, which would allocate 32 MiB for a
    // 20 KiB dex.
    let mut chunk = [0u8; 8 * 1024];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => bytes.extend_from_slice(&chunk[..read]),
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                logd!("Couldn't read '{}': {}", path, error);
                return None;
            }
        }
    }

    if bytes.len() as u64 > limit {
        logd!(
            "'{}' grew past the {} byte limit while reading",
            path,
            limit
        );
        return None;
    }
    Some(bytes)
}
