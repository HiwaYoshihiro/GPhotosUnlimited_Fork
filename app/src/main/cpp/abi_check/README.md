# Zygisk ABI cross-check

`zygisk.hpp` is vendored verbatim from LSPosed/Zygisk commit `a410e33`, the
revision the C++ implementation of this module was built against. It is only
used by `check.cpp`.

`check.cpp` asserts the field offsets and sizes that the hand-written Rust
bindings in `app/src/main/native/rust/src/zygisk.rs` assume. It is compiled by
the NDK for `arm64-v8a` and `armeabi-v7a` as an `OBJECT` library that is never
linked, so a mismatch fails the build instead of corrupting every
`preAppSpecialize` call at runtime.

The equivalent host-side assertions live in
`app/src/main/native/rust/tests/abi.rs`. They cannot catch the `armeabi-v7a`
case on a Windows host, where `long` is 4 bytes and pointers are 8; that is what
this directory covers.
