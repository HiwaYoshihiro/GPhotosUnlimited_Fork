# Google Photos Unlimited

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/HiwaYoshihiro/GPhotosUnlimited_Fork?label=Release&color=blue&style=flat)](https://github.com/HiwaYoshihiro/GPhotosUnlimited_Fork/releases/latest)
[![GitHub Release Date](https://img.shields.io/github/release-date/HiwaYoshihiro/GPhotosUnlimited_Fork?label=Release%20Date&color=brightgreen&style=flat)](https://github.com/HiwaYoshihiro/GPhotosUnlimited_Fork/releases)  
[![Main Releases](https://img.shields.io/github/downloads/HiwaYoshihiro/GPhotosUnlimited_Fork/latest/total?label=Main%20Repo%20Downloads%20%28Latest%20Release%29&color=blue&style=flat)](https://github.com/HiwaYoshihiro/GPhotosUnlimited_Fork/releases/latest)
[![Main All Releases](https://img.shields.io/github/downloads/HiwaYoshihiro/GPhotosUnlimited_Fork/total?label=Main%20Repo%20Total%20Downloads%20%28All%20Releases%29&color=brightgreen&style=flat)](https://github.com/HiwaYoshihiro/GPhotosUnlimited_Fork/releases)

A Zygisk module which gives unlimited Google Photos storage.

> [!IMPORTANT]
> This fork ships native code for ARM only: `arm64-v8a` and `armeabi-v7a`. The installer refuses to install on x86/x64 devices, where the module could not load.

To use this module you must have one of the following (latest versions):

- [Magisk](https://github.com/topjohnwu/Magisk) with Zygisk enabled
- [KernelSU](https://github.com/tiann/KernelSU) with [Zygisk Next](https://github.com/LSPosed/ZygiskNext) or [ReZygisk](https://github.com/PerformanC/ReZygisk) or [NeoZygisk](https://github.com/JingMatrix/NeoZygisk) module installed
- [KernelSU Next](https://github.com/KernelSU-Next/KernelSU-Next) with [Zygisk Next](https://github.com/LSPosed/ZygiskNext) or [ReZygisk](https://github.com/PerformanC/ReZygisk) or [NeoZygisk](https://github.com/JingMatrix/NeoZygisk) module installed
- [APatch](https://github.com/bmax121/APatch) with [Zygisk Next](https://github.com/LSPosed/ZygiskNext) or [ReZygisk](https://github.com/PerformanC/ReZygisk) or [NeoZygisk](https://github.com/JingMatrix/NeoZygisk) module installed

## About module

It injects a classes.dex file to modify fields in the android.os.Build class. It also hooks the native system-property reader and installs a per-process proxy over `IPackageManager` so `PackageManager.hasSystemFeature()` answers the Pixel-exclusive feature checks Google Photos uses to decide the backup tier. These are spoofed only into the Google Photos process, entirely in-process, without mounting or modifying any `/system` files, and with no per-device or per-ROM exception - the same answers are forced on every device and every ROM.

## Verifying unlimited storage

After installing (and rebooting), **back up a new photo**, then open that photo and swipe up (or tap the ⓘ button) to show its details. Working means the details say the photo does not count against your Google Account storage.

> [!IMPORTANT]
> Check a newly backed-up photo, **not** the settings screen. Google Photos does not reliably surface the storage tier under **Profile / Settings → Backup** any more - it is missing entirely in some versions and has moved or reappeared between others. Not seeing it there does not mean the module has failed, and this has cost people days of pointless troubleshooting.

If a newly backed-up photo *does* count against your storage, clear Google Photos app data and reboot, then take another photo and check again.

## Troubleshooting

Make sure Google Photos (com.google.android.apps.photos) is NOT on the Magisk DenyList if Enforce DenyList is enabled since this interferes with the module. The module no longer changes your DenyList for you; to have it remove Google Photos on each boot, create an empty file at `/data/adb/modules/unlimitedphotos/allow_denylist_fix`. That file is kept when the module is updated. Without it, check and change the DenyList yourself in the Magisk app, or with:

```sh
su -c "magisk --denylist rm com.google.android.apps.photos"
```

### Failing to work (on KernelSU/KernelSU Next/APatch)

- Disable Zygisk module
- Reboot
- Enable Zygisk module
- Reboot again

### Read module logs

You can read module logs using one of these commands directly after boot:

`adb shell "logcat | grep 'FGP/'"` or `su -c "logcat | grep 'FGP/'"`

Add a "verboseLogs" entry with a value of "0", "1", "2", "3" or "100" to your config to enable higher logging levels; "100" will dump all Build fields, all the system properties, and system features that Photos is checking. Because a custom file replaces `fgp.prop`, keep the other entries that are already in it.

## About spoofing Advanced Settings

The advanced spoofing options add granular control over what exactly gets spoofed, allowing one to disable the parts that may conflict with other kinds of spoofing modules. See more in the Details area below.

<details>
<summary><strong>Details</strong></summary>

- Other than for the "verboseLogs" entry (see above), they are all 0 (disabled) or 1 (enabled). In a JSON config, `false` and `true` are accepted as well.

- The "spoofBuild" entry (default 1) controls spoofing the Build Fields from the fingerprint; the "spoofProps" entry (default 0) controls spoofing the System Properties from the fingerprint; the "spoofProvider" entry (default 0) controls spoofing the Keystore Provider; the "spoofSignature" entry (default 0) controls spoofing the ROM Signature; and the "spoofFeatures" entry (default 1) controls spoofing the Pixel-exclusive features.

</details>

## Configuration files

The module reads the first readable, non-empty file in this order:

1. `/data/adb/modules/unlimitedphotos/custom.fgp.prop`
2. `/data/adb/modules/unlimitedphotos/custom.fgp.json`
3. the bundled `fgp.prop`

A custom file **replaces** the bundled file; it is not merged with it. Copy every entry you want to keep from `fgp.prop` (or start from `example.fgp.prop`), otherwise enabling only one spoof toggle can leave nothing to spoof. The module logs a warning when that happens.

Both formats accept the same keys. JSON additionally:

- may contain `//` and `/* */` comments;
- may use numbers and booleans instead of strings (`"SDK": 28`, `"PIXEL_EXPERIENCE": true`);
- may start with a UTF-8 BOM.

If a file looks like JSON (starts with `{` or `[`) but does not parse, the module logs the parser error and uses no config instead of misreading the file as `key=value` properties. Check `adb shell "logcat | grep 'FGP/'"` if a config seems to be ignored.

## Building

The native module is written in Rust; shadowhook (the inline-hook engine) stays C, and the dex-injected half is Java. You need the Android SDK/NDK, JDK 21 and a Rust toolchain with the two Android targets:

```sh
./gradlew assembleRelease
```

`rust-toolchain.toml` pins the exact toolchain, the two Android targets and the `rust-src` component, so `rustup` installs them automatically on the first build. The module zip lands in `out/GPhotosUnlimited.zip`; `module/module.prop` is the single source of the version shown in the zip and in the APK metadata.

By default the native build recompiles `std` from `rust-src` without its `backtrace` feature and at the release profile's `opt-level = "z"` (see `FGP_BUILD_STD` in `app/src/main/cpp/CMakeLists.txt`). That takes the native libraries from ~480 KB to ~248 KB (arm64) and ~326 KB to ~175 KB (armv7); panic messages are kept, backtraces are not. Pass `-PfgoBuildStd=false` to link the prebuilt `std` instead.

The release build runs `verifyDex`, which uses `dexdump` to check that R8 kept `CustomKeyStoreSpi`, `CustomProvider` and the other reflectively loaded classes. Run the remaining checks with:

```sh
cd app/src/main/native/rust
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cd -
./gradlew lintRelease testDebugUnitTest
```

Cargo is invoked from CMake with the NDK's own clang wrapper, derived from the toolchain the Gradle build already selected. If `cargo` is not on `PATH` (common on Windows right after installing Rust), CMake also checks `~/.cargo/bin` and you can override it with `-DCARGO_EXECUTABLE=...`.

Layout:

- `app/src/main/native/rust/` — the Rust module (`module.rs` for the Zygisk callbacks, `hook.rs` for the system property hook, `config.rs` for the config parsing, `setfield.rs` for the JNI helper).
- `app/src/main/cpp/` — the CMake glue and the export list; shadowhook is fetched at configure time. `abi_check/` holds a vendored `zygisk.hpp` and compile-time assertions that the Rust struct layouts match it on both ABIs.
- `app/src/main/java/` — the dex injected into Google Photos.

## Credits

Module scripts were adapted from those of osm0sis's Play Integrity Fork (PIFork) module, please see the commit history of [osm0sis's Play Integrity Fork](https://github.com/osm0sis/PlayIntegrityFork) for proper attribution.
