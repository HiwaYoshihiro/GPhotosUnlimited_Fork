cmake_minimum_required(VERSION 3.22.1)

project(unlimitedphotos C ASM)

# C++ is enabled only for the ABI cross-check translation unit at the bottom of
# this file; no C++ is linked into the module.
enable_language(CXX)

# The native module is Rust (see ../native/rust). shadowhook stays C: it is an
# inline-hook engine written against the ARM/ARM64 instruction encodings and has
# no Rust equivalent. No C++ is compiled any more, so libc++ is not needed.

set(RUST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../native/rust")

if(CMAKE_ANDROID_ARCH_ABI STREQUAL "arm64-v8a")
    set(RUST_TRIPLE "aarch64-linux-android")
    set(RUST_LINKER_ENV "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER")
    set(ARCH_LINK_FLAGS "-Wl,-z,max-page-size=16384")
elseif(CMAKE_ANDROID_ARCH_ABI STREQUAL "armeabi-v7a")
    set(RUST_TRIPLE "armv7-linux-androideabi")
    set(RUST_LINKER_ENV "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER")
    set(ARCH_LINK_FLAGS "")
else()
    message(FATAL_ERROR "Unsupported ABI: ${CMAKE_ANDROID_ARCH_ABI}")
endif()

if(NOT DEFINED CARGO_TARGET_DIR)
    set(CARGO_TARGET_DIR "${RUST_DIR}/target")
endif()

set(RUST_STATICLIB "${CARGO_TARGET_DIR}/${RUST_TRIPLE}/release/libunlimitedphotos.a")

# rustup installs to ~/.cargo/bin, which is not always on PATH (notably on
# Windows when the shell was started before the install, or in IDE builds).
# Check there explicitly before failing with a bare "not found".
find_program(CARGO_EXECUTABLE cargo
    HINTS
        "$ENV{CARGO_HOME}/bin"
        "$ENV{USERPROFILE}/.cargo/bin"
        "$ENV{HOME}/.cargo/bin"
)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR
        "cargo not found. Install Rust (https://rustup.rs) and make sure "
        "cargo is on PATH, or pass -DCARGO_EXECUTABLE=/path/to/cargo.")
endif()

# Cargo's own linker for the target. `CMAKE_C_COMPILER` is the generic
# clang.exe, not the per-target wrapper, so passing it would produce a host
# link. A `staticlib` needs no linker, which is why a wrong value here would
# stay invisible until someone runs `cargo build` by hand or adds a bin/test
# target.
if(CMAKE_ANDROID_ARCH_ABI STREQUAL "arm64-v8a")
    set(NDK_TARGET_TRIPLE "aarch64-linux-android")
else()
    set(NDK_TARGET_TRIPLE "armv7a-linux-androideabi")
endif()

set(NDK_CLANG "${CMAKE_ANDROID_NDK}/toolchains/llvm/prebuilt/${ANDROID_HOST_TAG}/bin/${NDK_TARGET_TRIPLE}${ANDROID_PLATFORM_LEVEL}-clang")
if(CMAKE_HOST_WIN32)
    # The NDK ships .cmd shims on Windows; the extensionless file is a shell
    # script that cargo cannot exec directly.
    set(NDK_CLANG "${NDK_CLANG}.cmd")
endif()
if(NOT EXISTS "${NDK_CLANG}")
    message(FATAL_ERROR "NDK clang wrapper not found: ${NDK_CLANG}")
endif()

if(NOT CMAKE_C_COMPILER)
    message(FATAL_ERROR "CMAKE_C_COMPILER is not set; the NDK toolchain did not run")
endif()

include(FetchContent)

# Pinned to the v2.0.1 commit and verified by content hash. A movable tag would
# let a changed upstream rewrite what gets linked into the module.
FetchContent_Declare(
  shadowhook
  URL      https://github.com/bytedance/android-inline-hook/archive/e6b23d6883feab6052feff8e3e6959d710845a92.tar.gz
  URL_HASH SHA256=8ceb6b58c684e2ae8b3b7376a60ab500a21bb6f2f4e0cd33abe8df99da2e3de3
)
FetchContent_GetProperties(shadowhook)
if(NOT shadowhook_POPULATED)
  FetchContent_Populate(shadowhook)
endif()

file(GLOB SHADOWHOOK_SRC
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/*.c
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/common/*.c
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/third_party/xdl/*.c
)

if(CMAKE_ANDROID_ARCH_ABI STREQUAL "armeabi-v7a")
    file(GLOB SHADOWHOOK_ARCH_SRC ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm/*.c ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm/*.S)
    include_directories(${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm)
elseif(CMAKE_ANDROID_ARCH_ABI STREQUAL "arm64-v8a")
    file(GLOB SHADOWHOOK_ARCH_SRC ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm64/*.c ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm64/*.S)
    include_directories(${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/arch/arm64)
endif()

include_directories(
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/common
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/include
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/third_party/bsd
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/third_party/lss
    ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/third_party/xdl
)

# One custom target, so cargo runs once per build instead of once per source.
add_custom_target(unlimitedphotos_rust ALL
    COMMAND ${CMAKE_COMMAND} -E env
            "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"
            "${RUST_LINKER_ENV}=${NDK_CLANG}"
            ${CARGO_EXECUTABLE} build --locked --release --target ${RUST_TRIPLE}
            --manifest-path ${RUST_DIR}/Cargo.toml
    BYPRODUCTS ${RUST_STATICLIB}
    COMMENT "Building Rust native module for ${RUST_TRIPLE}"
    VERBATIM
)

add_library(unlimitedphotos_rust_core STATIC IMPORTED GLOBAL)
set_target_properties(unlimitedphotos_rust_core PROPERTIES
    IMPORTED_LOCATION "${RUST_STATICLIB}"
)
add_dependencies(unlimitedphotos_rust_core unlimitedphotos_rust)

add_library(${CMAKE_PROJECT_NAME} SHARED
    ${SHADOWHOOK_SRC}
    ${SHADOWHOOK_ARCH_SRC}
)

# shadowhook calls sh_linker_init() during shadowhook_init(). We provide our own
# no-op for it (../native/rust/src/lib.rs), because hooking the linker's soinfo
# ctor/dtor walk is unnecessary here and is one more thing to be fingerprinted.
#
# The rename must apply to the *definition* only. Defining it for the whole
# target would also rewrite shadowhook.c's call site, so shadowhook would keep
# calling the real linker hook under its new name and the Rust stub would be
# dead code. Scoping it to sh_linker.c leaves the call site intact.
set(SHADOWHOOK_LINKER_C ${shadowhook_SOURCE_DIR}/shadowhook/src/main/cpp/sh_linker.c)
set_source_files_properties(${SHADOWHOOK_LINKER_C} PROPERTIES
    COMPILE_DEFINITIONS "sh_linker_init=sh_linker_init_dummy"
)

# Nothing in the C sources references the Rust entry points, so the archive
# members that define them would be dropped. `-u` pulls in exactly those objects
# (and whatever they need) instead of `--whole-archive`, which would also drag in
# Rust's bundled compiler-builtins and collide with the NDK's libclang_rt
# definitions of the same __aeabi_* helpers on armeabi-v7a.
target_link_libraries(${CMAKE_PROJECT_NAME} PRIVATE
    "-Wl,-u,zygisk_module_entry"
    "-Wl,-u,zygisk_companion_entry"
    "-Wl,-u,sh_linker_init"
    unlimitedphotos_rust_core
    log
    dl
    m
)

# Export only what Zygisk looks up. Rust's std and shadowhook's internals would
# otherwise land in .dynsym, which bloats the module and widens the surface an
# app can inspect.
target_link_options(${CMAKE_PROJECT_NAME} PRIVATE
    "-Wl,--version-script=${CMAKE_CURRENT_SOURCE_DIR}/unlimitedphotos.map.txt"
    "-Wl,--gc-sections"
    ${ARCH_LINK_FLAGS}
)

set_target_properties(${CMAKE_PROJECT_NAME} PROPERTIES
    C_VISIBILITY_PRESET hidden
)

# Compile-time ABI cross-check against the real zygisk.hpp, for the target that
# is actually being built. The object library is built but never linked: it
# exists so the `static_assert`s fail the build if the hand-written Rust
# bindings drift from the C++ header on this ABI. `tests/abi.rs` covers the same
# numbers, but only on the host, where `long` and pointer sizes differ from
# armeabi-v7a.
add_library(unlimitedphotos_abi_check OBJECT abi_check/check.cpp)
set_target_properties(unlimitedphotos_abi_check PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
    CXX_EXTENSIONS OFF
    CXX_VISIBILITY_PRESET hidden
)
# No exceptions or RTTI: the header does not need them, and this keeps the TU
# from pulling in libc++.
target_compile_options(unlimitedphotos_abi_check PRIVATE
    -fno-exceptions
    -fno-rtti
)
target_include_directories(unlimitedphotos_abi_check PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/abi_check
)
add_dependencies(${CMAKE_PROJECT_NAME} unlimitedphotos_abi_check)
