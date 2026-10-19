// Compile-time cross-check of the hand-written Rust bindings against the real
// `zygisk.hpp`.
//
// `zygisk.hpp` is vendored verbatim from LSPosed/Zygisk commit a410e33, the same
// header the C++ module was built against; it is not part of the build output.
//
// The Rust side asserts the same numbers in `tests/abi.rs`, but that test only
// runs on the host, where `sizeof(void *)` is 8 and `long` is 4 on Windows. This
// translation unit is compiled by the NDK for both `arm64-v8a` and
// `armeabi-v7a`, so a mismatch on either real target fails the build instead of
// crashing an app process at runtime.
//
// Nothing here is linked into the module: it exists purely for the
// `static_assert`s, which is why the object library is never added to the
// shared target.

#include <cstddef>
#include <cstdint>

// The header's declarations are all this TU needs; no definition is referenced.
#include "zygisk.hpp"

namespace {

using zygisk::AppSpecializeArgs;
using zygisk::ServerSpecializeArgs;
using zygisk::internal::api_table;
using zygisk::internal::module_abi;

constexpr std::size_t PTR = sizeof(void *);

// `offsetof` is only conditionally supported for non-standard-layout types, and
// these structs hold references, so it warns here. Clang computes the offsets
// correctly for this target; the alternative (building a fake object) is
// impossible because both constructors are deleted.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Winvalid-offsetof"

// --- module_abi -----------------------------------------------------------
// Rust: ModuleAbi in src/zygisk.rs.

static_assert(offsetof(module_abi, api_version) == 0);
static_assert(offsetof(module_abi, impl) == PTR);
static_assert(offsetof(module_abi, preAppSpecialize) == 2 * PTR);
static_assert(offsetof(module_abi, postAppSpecialize) == 3 * PTR);
static_assert(offsetof(module_abi, preServerSpecialize) == 4 * PTR);
static_assert(offsetof(module_abi, postServerSpecialize) == 5 * PTR);
static_assert(sizeof(module_abi) == 6 * PTR);

// `long` is 4 bytes on armeabi-v7a and 8 on arm64. Hardcoding a width in Rust
// would shift every following pointer on one of the two targets.
static_assert(sizeof(module_abi::api_version) == sizeof(long));

// --- AppSpecializeArgs ----------------------------------------------------
// Rust: AppSpecializeArgs in src/zygisk.rs. Each `jint &` is a pointer.

static_assert(offsetof(AppSpecializeArgs, uid) == 0);
static_assert(offsetof(AppSpecializeArgs, gid) == 1 * PTR);
static_assert(offsetof(AppSpecializeArgs, gids) == 2 * PTR);
static_assert(offsetof(AppSpecializeArgs, runtime_flags) == 3 * PTR);
static_assert(offsetof(AppSpecializeArgs, mount_external) == 4 * PTR);
static_assert(offsetof(AppSpecializeArgs, se_info) == 5 * PTR);
static_assert(offsetof(AppSpecializeArgs, nice_name) == 6 * PTR);
static_assert(offsetof(AppSpecializeArgs, instruction_set) == 7 * PTR);
static_assert(offsetof(AppSpecializeArgs, app_data_dir) == 8 * PTR);
static_assert(offsetof(AppSpecializeArgs, is_child_zygote) == 9 * PTR);
static_assert(offsetof(AppSpecializeArgs, is_top_app) == 10 * PTR);
static_assert(offsetof(AppSpecializeArgs, pkg_data_info_list) == 11 * PTR);
static_assert(offsetof(AppSpecializeArgs, whitelisted_data_info_list) == 12 * PTR);
static_assert(offsetof(AppSpecializeArgs, mount_data_dirs) == 13 * PTR);
static_assert(offsetof(AppSpecializeArgs, mount_storage_dirs) == 14 * PTR);
static_assert(sizeof(AppSpecializeArgs) == 15 * PTR);

// --- ServerSpecializeArgs -------------------------------------------------
// Rust: ServerSpecializeArgs in src/zygisk.rs.

static_assert(offsetof(ServerSpecializeArgs, uid) == 0);
static_assert(offsetof(ServerSpecializeArgs, gid) == 1 * PTR);
static_assert(offsetof(ServerSpecializeArgs, gids) == 2 * PTR);
static_assert(offsetof(ServerSpecializeArgs, runtime_flags) == 3 * PTR);
static_assert(offsetof(ServerSpecializeArgs, permitted_capabilities) == 4 * PTR);
static_assert(offsetof(ServerSpecializeArgs, effective_capabilities) == 5 * PTR);
static_assert(sizeof(ServerSpecializeArgs) == 6 * PTR);

// --- api_table ------------------------------------------------------------
// Rust: ApiTable in src/zygisk.rs.

static_assert(offsetof(api_table, impl) == 0);
static_assert(offsetof(api_table, registerModule) == 1 * PTR);
static_assert(offsetof(api_table, hookJniNativeMethods) == 2 * PTR);
static_assert(offsetof(api_table, pltHookRegister) == 3 * PTR);
static_assert(offsetof(api_table, pltHookExclude) == 4 * PTR);
static_assert(offsetof(api_table, pltHookCommit) == 5 * PTR);
static_assert(offsetof(api_table, connectCompanion) == 6 * PTR);
static_assert(offsetof(api_table, setOption) == 7 * PTR);
static_assert(offsetof(api_table, getModuleDir) == 8 * PTR);
static_assert(offsetof(api_table, getFlags) == 9 * PTR);
static_assert(sizeof(api_table) == 10 * PTR);

// --- Option enum ----------------------------------------------------------
// Rust: FORCE_DENYLIST_UNMOUNT / DLCLOSE_MODULE_LIBRARY in src/zygisk.rs.

static_assert(static_cast<int>(zygisk::FORCE_DENYLIST_UNMOUNT) == 0);
static_assert(static_cast<int>(zygisk::DLCLOSE_MODULE_LIBRARY) == 1);

#pragma clang diagnostic pop

}  // namespace
