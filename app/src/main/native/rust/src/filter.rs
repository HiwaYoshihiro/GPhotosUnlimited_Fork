//! Which app processes the module is allowed to act on.
//!
//! `pre_app_specialize` runs in *every* app process, so the target check has to
//! be exact. A looser `contains()`/`starts_with()` match would let a lookalike
//! package such as `com.google.android.apps.photos.extra`, or a secondary
//! Photos process, reach the hook and the dex injection.
//!
//! Kept out of `module.rs` (which is Android-only) so the rules can be tested on
//! a host machine.

/// Package name of Google Photos.
pub const PHOTOS_PACKAGE: &str = "com.google.android.apps.photos";

/// True when `app_data_dir` is the data directory of [`PHOTOS_PACKAGE`].
///
/// Android passes `/data/user/<id>/<package>` (or `/data/user_de/<id>/<package>`
/// for the device-encrypted one), so the package name must be the last path
/// segment. `strip_suffix` alone is not enough: it would also accept
/// `com.google.android.apps.photos.extra`.
pub fn is_photos_data_dir(app_data_dir: &str) -> bool {
    app_data_dir
        .strip_suffix(PHOTOS_PACKAGE)
        .is_some_and(|prefix| prefix.ends_with('/'))
}

/// True only for the main Google Photos process.
///
/// Secondary processes are named `<package>:<suffix>` (`:crashpad`,
/// `:scheduler`, ...) and must not be injected into.
pub fn is_photos_main_process(process: &str) -> bool {
    process == PHOTOS_PACKAGE
}
