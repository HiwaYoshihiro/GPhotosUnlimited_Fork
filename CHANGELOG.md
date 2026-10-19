## Unreleased

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
