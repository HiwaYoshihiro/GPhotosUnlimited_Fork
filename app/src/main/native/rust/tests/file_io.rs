//! Bounded file reads.
//!
//! The cap has to hold against a file that is bigger than the limit, and it has
//! to keep holding while the file changes size. These drive the reader on a
//! real file system, which is the only place `File::open` and `take` can be
//! observed.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use unlimitedphotos::file_io::read_capped;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let name = format!(
            "unlimitedphotos-file-io-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("write temp file");
        path.to_str().expect("utf-8 temp path").to_string()
    }

    fn dir(&self, name: &str) -> String {
        let path = self.0.join(name);
        fs::create_dir_all(&path).expect("create temp subdir");
        path.to_str().expect("utf-8 temp path").to_string()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_at_the_limit_is_accepted() {
    let dir = TempDir::new();
    let path = dir.file("exact", b"12345");
    assert_eq!(read_capped(&path, 5).as_deref(), Some(b"12345".as_slice()));
}

#[test]
fn a_file_one_byte_over_the_limit_is_rejected() {
    // This is the case `take(limit + 1)` exists to catch: the metadata may say
    // one thing, but the read itself stops one byte past the limit and the
    // result is refused.
    let dir = TempDir::new();
    let path = dir.file("over", b"123456");
    assert!(read_capped(&path, 5).is_none());
}

#[test]
fn a_file_far_over_the_limit_is_rejected_without_reading_it() {
    let dir = TempDir::new();
    let path = dir.file("huge", &vec![0u8; 64 * 1024]);
    assert!(read_capped(&path, 1024).is_none());
}

#[test]
fn a_missing_file_is_not_an_error() {
    let dir = TempDir::new();
    let path = dir.0.join("absent");
    assert!(read_capped(path.to_str().unwrap(), 16).is_none());
}

#[test]
fn a_directory_is_rejected() {
    let dir = TempDir::new();
    let path = dir.dir("nested");
    assert!(read_capped(&path, 1024).is_none());
}

#[test]
fn an_empty_file_is_accepted() {
    let dir = TempDir::new();
    let path = dir.file("empty", b"");
    assert_eq!(read_capped(&path, 16).as_deref(), Some(b"".as_slice()));
}

#[test]
fn a_zero_limit_only_accepts_an_empty_file() {
    let dir = TempDir::new();
    let empty = dir.file("empty", b"");
    let one = dir.file("one", b"x");
    assert_eq!(read_capped(&empty, 0).as_deref(), Some(b"".as_slice()));
    assert!(read_capped(&one, 0).is_none());
}
