//! ABI layout checks for the hand-written Zygisk bindings.
//!
//! These assert the assumptions `zygisk.hpp` made implicitly. `AppSpecializeArgs`
//! is a struct of references in C++ (`jint &uid` -> `*mut jint`), and
//! `module_abi.api_version` is `long`, which is 4 bytes on armeabi-v7a. Getting
//! either wrong shifts every argument Zygisk passes and crashes the app process
//! before the module can bail out.
//!
//! Field offsets are asserted rather than total size, because the C++ struct is
//! padded to pointer alignment: on a 64-bit target `long` + padding occupies
//! the same 8 bytes as a pointer.

use core::ffi::c_long;
use core::mem::{align_of, offset_of, size_of};

use unlimitedphotos::zygisk::{ApiTable, AppSpecializeArgs, ModuleAbi, ServerSpecializeArgs};

#[test]
fn module_abi_field_offsets_match_the_cpp_struct() {
    let ptr = size_of::<*const ()>();

    assert_eq!(offset_of!(ModuleAbi, api_version), 0);
    assert_eq!(offset_of!(ModuleAbi, impl_), ptr);
    assert_eq!(offset_of!(ModuleAbi, pre_app_specialize), 2 * ptr);
    assert_eq!(offset_of!(ModuleAbi, post_app_specialize), 3 * ptr);
    assert_eq!(offset_of!(ModuleAbi, pre_server_specialize), 4 * ptr);
    assert_eq!(offset_of!(ModuleAbi, post_server_specialize), 5 * ptr);
    assert_eq!(size_of::<ModuleAbi>(), 6 * ptr);
}

#[test]
fn app_specialize_args_field_offsets_match_the_cpp_struct() {
    let ptr = size_of::<*const ()>();

    // Nine required references, then six optional pointers, in header order.
    assert_eq!(offset_of!(AppSpecializeArgs, uid), 0);
    assert_eq!(offset_of!(AppSpecializeArgs, gid), ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, gids), 2 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, runtime_flags), 3 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, mount_external), 4 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, se_info), 5 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, nice_name), 6 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, instruction_set), 7 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, app_data_dir), 8 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, is_child_zygote), 9 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, is_top_app), 10 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, pkg_data_info_list), 11 * ptr);
    assert_eq!(
        offset_of!(AppSpecializeArgs, whitelisted_data_info_list),
        12 * ptr
    );
    assert_eq!(offset_of!(AppSpecializeArgs, mount_data_dirs), 13 * ptr);
    assert_eq!(offset_of!(AppSpecializeArgs, mount_storage_dirs), 14 * ptr);
    assert_eq!(size_of::<AppSpecializeArgs>(), 15 * ptr);
}

#[test]
fn server_specialize_args_field_offsets_match_the_cpp_struct() {
    let ptr = size_of::<*const ()>();

    assert_eq!(offset_of!(ServerSpecializeArgs, uid), 0);
    assert_eq!(offset_of!(ServerSpecializeArgs, gid), ptr);
    assert_eq!(offset_of!(ServerSpecializeArgs, gids), 2 * ptr);
    assert_eq!(offset_of!(ServerSpecializeArgs, runtime_flags), 3 * ptr);
    assert_eq!(
        offset_of!(ServerSpecializeArgs, permitted_capabilities),
        4 * ptr
    );
    assert_eq!(
        offset_of!(ServerSpecializeArgs, effective_capabilities),
        5 * ptr
    );
    assert_eq!(size_of::<ServerSpecializeArgs>(), 6 * ptr);
}

#[test]
fn api_table_field_offsets_match_the_cpp_struct() {
    let ptr = size_of::<*const ()>();

    // impl, registerModule, hookJniNativeMethods, pltHookRegister,
    // pltHookExclude, pltHookCommit, connectCompanion, setOption,
    // getModuleDir, getFlags.
    assert_eq!(offset_of!(ApiTable, impl_), 0);
    assert_eq!(offset_of!(ApiTable, register_module), ptr);
    assert_eq!(offset_of!(ApiTable, hook_jni_native_methods), 2 * ptr);
    assert_eq!(offset_of!(ApiTable, plt_hook_register), 3 * ptr);
    assert_eq!(offset_of!(ApiTable, plt_hook_exclude), 4 * ptr);
    assert_eq!(offset_of!(ApiTable, plt_hook_commit), 5 * ptr);
    assert_eq!(offset_of!(ApiTable, connect_companion), 6 * ptr);
    assert_eq!(offset_of!(ApiTable, set_option), 7 * ptr);
    assert_eq!(offset_of!(ApiTable, get_module_dir), 8 * ptr);
    assert_eq!(offset_of!(ApiTable, get_flags), 9 * ptr);
    assert_eq!(size_of::<ApiTable>(), 10 * ptr);
}

#[test]
fn api_version_uses_c_long_not_a_fixed_width_integer() {
    // The C++ header declares `long api_version`. On armeabi-v7a that is 4
    // bytes; hardcoding i64 there would push every following pointer 4 bytes
    // out and Zygisk would call through a garbage address.
    assert_eq!(size_of::<ModuleAbi>(), 6 * size_of::<*const ()>());
    assert!(align_of::<ModuleAbi>() <= align_of::<*const ()>());
    // `c_long` is what the ABI calls for; this is the property that matters.
    assert_eq!(size_of::<c_long>(), size_of::<libc::c_long>());
}

#[test]
fn parse_int_matches_the_cpp_hand_rolled_parser() {
    use unlimitedphotos::config::parse_int;

    assert_eq!(parse_int("0"), Some(0));
    assert_eq!(parse_int("1"), Some(1));
    assert_eq!(parse_int("100"), Some(100));
    assert_eq!(parse_int("-1"), Some(-1));
    assert_eq!(parse_int("+2"), Some(2));

    // Rejected by the C++ loop, so still rejected here.
    assert_eq!(parse_int(""), None);
    assert_eq!(parse_int("-"), None);
    assert_eq!(parse_int("1 "), None);
    assert_eq!(parse_int(" 1"), None);
    assert_eq!(parse_int("0x1"), None);
    assert_eq!(parse_int("1.0"), None);
    assert_eq!(parse_int("true"), None);
}

#[test]
fn prop_config_parses_like_the_cpp_converter() {
    use serde_json::Value;
    use unlimitedphotos::config::parse_config;

    let text = "# comment\r\nMANUFACTURER=Google\r\nMODEL=Pixel # trailing\r\n\r\n  SDK = 28  \r\n";
    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(
        map.get("MANUFACTURER"),
        Some(&Value::String("Google".into()))
    );
    assert_eq!(map.get("MODEL"), Some(&Value::String("Pixel".into())));
    // Trimmed on both sides, like the C++ `trim()`.
    assert_eq!(map.get("SDK"), Some(&Value::String("28".into())));
}

#[test]
fn json_config_keeps_comments_and_rejects_prop_format() {
    use serde_json::Value;
    use unlimitedphotos::config::parse_config;

    // nlohmann was called with ignore_comments=true, so a commented JSON config
    // must not be mistaken for the prop format.
    let text = r#"{
        // advanced settings
        "spoofProps": "1",
        /* feature override */
        "com.google.android.feature.PIXEL_EXPERIENCE": "true"
    }"#;
    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(map.len(), 2);
    assert_eq!(map.get("spoofProps"), Some(&Value::String("1".into())));
}

#[test]
fn json_config_preserves_quotes_and_backslashes_in_values() {
    use serde_json::Value;
    use unlimitedphotos::config::parse_config;

    // The C++ path built JSON by string concatenation, so a quote in a value
    // broke the whole config. This must survive intact.
    let text = r#"{"*.security_patch": "a\"b\\c"}"#;
    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(
        map.get("*.security_patch"),
        Some(&Value::String("a\"b\\c".into()))
    );
}

#[test]
fn settings_are_removed_and_properties_split_out() {
    use serde_json::Value;
    use unlimitedphotos::config::{apply_settings, parse_config, Settings};

    let Value::Object(mut map) = parse_config(
        r#"{
            "verboseLogs": "1",
            "spoofProps": "1",
            "*.security_patch": "2018-08-05",
            "MANUFACTURER": "Google",
            "com.google.android.feature.PIXEL_EXPERIENCE": "true"
        }"#,
    ) else {
        panic!("expected an object");
    };

    let mut settings = Settings::default();
    let props = apply_settings(&mut map, &mut settings);

    assert_eq!(settings.verbose_logs, 1);
    assert_eq!(settings.spoof_props, 1);
    // Property entry moved into the hook table, feature and Build field stay.
    assert_eq!(
        props.get("*.security_patch").map(String::as_str),
        Some("2018-08-05")
    );
    assert!(!map.contains_key("*.security_patch"));
    assert!(map.contains_key("MANUFACTURER"));
    assert!(map.contains_key("com.google.android.feature.PIXEL_EXPERIENCE"));
    assert!(!map.contains_key("verboseLogs"));
}

#[test]
fn wildcard_matching_picks_the_sorted_first_match_like_std_map() {
    use std::collections::BTreeMap;

    // Both patterns match, so the winner is decided purely by iteration order.
    // The old code walked `std::map` (sorted), where "*patch" comes before
    // "*security_patch". With a HashMap the answer would vary per process, so
    // the spoofed value would flip between runs.
    let mut props: BTreeMap<String, String> = BTreeMap::new();
    props.insert("*security_patch".into(), "specific".into());
    props.insert("*patch".into(), "generic".into());

    let winner = props
        .iter()
        .find(|(key, _)| {
            key.strip_prefix('*')
                .is_some_and(|suffix| "ro.build.security_patch".ends_with(suffix))
        })
        .map(|(_, value)| value.as_str());

    assert_eq!(winner, Some("generic"));
}
