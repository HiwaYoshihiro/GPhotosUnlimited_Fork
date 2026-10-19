## Unreleased

- Port the native module from C++ to Rust. Behaviour, config files, log tags and the module layout are unchanged. shadowhook remains C and the dex-injected half remains Java.
- Drop the `local_cxa_atexit_finalize_impl` submodule and the `nlohmann/json` submodule: Rust has no C++ static destructors to hide from the global `AtexitEntry` array, and `serde_json` replaces the bundled JSON library.
- Drop the `org.lsposed.libcxx` dependency, no longer needed now that no C++ is compiled.
- JSON configs keep accepting `//` and `/* */` comments, as before.

## Google Photos Unlimited v5

- Fix features (e.g. Unblur), only the features listed in `fgp.prop` are overridden and everything else passes through ([#20](https://github.com/Rev4N1/GPhotosUnlimited/issues/20))
- Make Pixel feature spoofing configurable: add a `# Feature Overrides` section to `fgp.prop` so any `hasSystemFeature` query can be set to `true`/`false`
- Avoid future detection of Dobby by switching spoofProps libc hooking method to latest ShadowHook (thanks @pershoot)
- Fix restoring from module versions up to v3: also remove stale `pixel_*_exclusive.xml` overrides and fully clean up the empty `$MODPATH/system` directory
- Drop dead zygisk guards in `customize.sh`/`post-fs-data.sh` (zygisk has been required since rebase)

_[Full changelogs](https://github.com/Rev4N1/GPhotosUnlimited/releases)_
