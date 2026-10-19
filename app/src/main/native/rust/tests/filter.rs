//! Target-process selection.
//!
//! `pre_app_specialize` runs in every app process on the device, so these rules
//! decide whether the module touches a process at all. The failure mode of a
//! loose match is not cosmetic: the hook and the dex injection would run inside
//! an unrelated app.

use unlimitedphotos::filter::{is_photos_data_dir, is_photos_main_process};

#[test]
fn accepts_the_main_photos_process() {
    assert!(is_photos_main_process("com.google.android.apps.photos"));
    assert!(is_photos_data_dir(
        "/data/user/0/com.google.android.apps.photos"
    ));
    // Device-encrypted storage uses the same trailing package name.
    assert!(is_photos_data_dir(
        "/data/user_de/0/com.google.android.apps.photos"
    ));
    // A secondary user id must still be accepted: the id is not part of the
    // package name.
    assert!(is_photos_data_dir(
        "/data/user/10/com.google.android.apps.photos"
    ));
}

#[test]
fn rejects_secondary_photos_processes() {
    // These share the data directory, so only the process-name check keeps the
    // module out of them.
    for process in [
        "com.google.android.apps.photos:crashpad",
        "com.google.android.apps.photos:scheduler",
        "com.google.android.apps.photos:remote",
        "com.google.android.apps.photos:",
    ] {
        assert!(
            !is_photos_main_process(process),
            "{process} must not be treated as the main process"
        );
    }
}

#[test]
fn rejects_lookalike_package_names() {
    for process in [
        "com.google.android.apps.photos.extra",
        "com.google.android.apps.photos.backup",
        "com.google.android.apps.photos2",
        "com.google.android.apps.photo",
        "photos",
        "",
    ] {
        assert!(
            !is_photos_main_process(process),
            "{process} must not be treated as the main process"
        );
    }
}

#[test]
fn rejects_lookalike_data_directories() {
    for dir in [
        // Suffix match without a path boundary: the old `ends_with` form
        // accepted this, `strip_suffix` + '/' rejects it.
        "/data/user/0/evil.com.google.android.apps.photos",
        "/data/user/0/com.google.android.apps.photos.extra",
        "/data/user/0/com.google.android.apps.photos.backup",
        "/data/user/0/com.google.android.apps.photo",
        "/data/user/0/com.google.android.apps.photos/",
        "/data/user/0/",
        "/data/user/0",
        "com.google.android.apps.photos",
        "",
    ] {
        assert!(
            !is_photos_data_dir(dir),
            "{dir} must not be treated as the Photos data directory"
        );
    }
}

#[test]
fn rejects_other_packages_and_system_server() {
    assert!(!is_photos_main_process("android"));
    assert!(!is_photos_main_process("system_server"));
    assert!(!is_photos_main_process("com.android.settings"));
    assert!(!is_photos_data_dir("/data/user/0/com.android.settings"));
    assert!(!is_photos_data_dir("/data/system"));
}
