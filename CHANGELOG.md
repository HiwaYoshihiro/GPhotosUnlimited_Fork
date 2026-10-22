## Google Photos Unlimited v6.2

- Fix the ProGuard rule that let R8 strip `CustomKeyStoreSpi` down to an empty abstract class. The dex installed fine, but `spoofProvider` registered a class with no `engine*` methods, so every AndroidKeyStore operation failed. `CustomProvider`, `CustomPackageInfoCreator` and `CustomPackageManager` are now explicitly kept as well.
- Add a `verifyDex` build check that fails when a reflectively loaded class or method is missing from the shipped dex.
- Run clippy against `aarch64-linux-android` and `armv7-linux-androideabi` in CI, not just the host. This caught an `unnecessary_cast` in `hook.rs` that only exists in Android code.
- Accept JSON numbers and booleans for Advanced Settings, Build fields, properties and feature overrides, not just strings.
- Ignore a UTF-8 BOM in config files, and refuse to reinterpret broken JSON as `key=value` properties.
- Ignore an empty custom config instead of letting it shadow the bundled `fgp.prop`, and warn when a custom config leaves nothing to spoof.
- Make `EntryPoint.setField()` tolerate per-field failures (inaccessible fields, bad numbers) instead of aborting the remaining spoofing.
- Answer configured feature overrides before calling the PackageManager binder, and restore the original keystore provider if installing the custom one fails.
- Preserve `allow_denylist_fix` when updating the module, refuse to install on unsupported architectures, and guard the `magisk` command used by `post-fs-data.sh`.
- Use `HARDWARE=sailfish` in the bundled Pixel profile (it said `marlin`, the Pixel XL codename).
- Pin shadowhook to the v2.0.1 commit with a SHA-256 content hash, build Cargo with `--locked`, pin the Rust toolchain, and add Gradle wrapper verification.
- Remove stale `module/zygisk` outputs before packaging, upload the actual module zip from CI, and validate its contents.
- Read the module version from `module.prop` in Gradle, align `Cargo.toml`, fix the README badges, and document architecture support and config precedence.
- Add Java unit tests, lint, shellcheck and Dependabot checks to CI.

## Google Photos Unlimited v6.1

- Build the strings handed to the dex-injected half with `NewString` and UTF-16 instead of `NewStringUTF`. `NewStringUTF` expects modified UTF-8, so a config value containing a character outside the BMP (an emoji, for example) was passed outside the JNI contract.
- Clear a pending JNI exception when string creation fails. Both call sites logged a null return and continued, which left the exception set for the next JNI call made on that thread.
- Make the companion's size limit a hard cap. The size was checked with `fs::metadata` and then read with `fs::read`, so a file that grew or was replaced in between was still read in full.

## Google Photos Unlimited v6

- Port the native module from C++ to Rust. Behaviour, config files, log tags and the module layout are unchanged. shadowhook remains C and the dex-injected half remains Java.
- Drop the `local_cxa_atexit_finalize_impl` submodule and the `nlohmann/json` submodule: Rust has no C++ static destructors to hide from the global `AtexitEntry` array, and `serde_json` replaces the bundled JSON library.
- Drop the `org.lsposed.libcxx` dependency, no longer needed now that no C++ is compiled.
- JSON configs keep accepting `//` and `/* */` comments, as before.
- Stop hiding conflicting custom ROM injection apps. The module no longer manages a `system` tree, so it no longer changes the mount namespace of apps other than Google Photos. `app_replace_list.txt` is removed; a leftover `custom.app_replace_list.txt` is ignored and can be deleted.
- Do not carry `$MODPATH/system` from an older version into the new one. Existing overrides are removed during the update and a reboot is required for that to take effect.
- Stop removing Google Photos from the Magisk DenyList on every boot. That silently changed a root setting; the script now only does it when `allow_denylist_fix` exists in the module directory.
- Reject out-of-range numbers in the Advanced Settings instead of wrapping them, and check each spoof toggle separately so a negative value cannot cancel a positive one.
- Fix multi-byte UTF-8 characters being corrupted in a JSON config that contains comments.
- Release the JNI local references `inject()` creates on every failure path, not just on success.
- Keep the dex buffer that the in-memory classloader reads alive for the life of the process. The C++ version did this by accident (`std::vector::clear()` does not free the allocation); dropping it after injection could leave ART reading freed memory.
- Check the result of `shadowhook_init()`, and only log spoofed properties when `verboseLogs` is enabled.

## Google Photos Unlimited v5

- Fix features (e.g. Unblur), only the features listed in `fgp.prop` are overridden and everything else passes through ([#20](https://github.com/Rev4N1/GPhotosUnlimited/issues/20))
- Make Pixel feature spoofing configurable: add a `# Feature Overrides` section to `fgp.prop` so any `hasSystemFeature` query can be set to `true`/`false`
- Avoid future detection of Dobby by switching spoofProps libc hooking method to latest ShadowHook (thanks @pershoot)
- Fix restoring from module versions up to v3: also remove stale `pixel_*_exclusive.xml` overrides and fully clean up the empty `$MODPATH/system` directory
- Drop dead zygisk guards in `customize.sh`/`post-fs-data.sh` (zygisk has been required since rebase)

_[Full changelogs](https://github.com/Rev4N1/GPhotosUnlimited/releases)_
