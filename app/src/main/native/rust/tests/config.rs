//! Config ingestion edge cases: BOM handling, JSON that must not silently fall
//! back to the prop parser, and scalar values other than strings.

use serde_json::{json, Value};
use unlimitedphotos::config::{apply_settings, parse_config, Settings};

#[test]
fn utf8_bom_is_ignored() {
    let Value::Object(map) = parse_config("\u{feff}{\"MODEL\": \"Pixel\"}") else {
        panic!("expected an object");
    };
    assert_eq!(map.get("MODEL"), Some(&json!("Pixel")));
}

#[test]
fn broken_json_is_not_reinterpreted_as_props() {
    // A trailing comma is a common JSON typo. Running this through the prop
    // parser would produce nonsense property names and spoof unrelated values.
    let value = parse_config("{\n  \"MODEL\": \"Pixel\",\n}");
    let Value::Object(map) = value else {
        panic!("expected an object");
    };
    assert!(map.is_empty());
}

#[test]
fn json_numbers_and_booleans_are_valid_settings() {
    let mut value = parse_config(
        r#"{
            "spoofBuild": 0,
            "spoofFeatures": true,
            "SDK": 28,
            "PIXEL": true
        }"#,
    );
    let Value::Object(ref mut json) = value else {
        panic!("expected an object");
    };
    let mut settings = Settings::default();

    let props = apply_settings(json, &mut settings);

    assert_eq!(settings.spoof_build, 0);
    assert_eq!(settings.spoof_features, 1);
    assert!(props.is_empty());
    assert_eq!(json.get("SDK"), Some(&json!(28)));
    assert_eq!(json.get("PIXEL"), Some(&json!(true)));
}

#[test]
fn json_numbers_and_booleans_are_valid_properties() {
    let mut value = parse_config(r#"{"*.security_patch": 20180805, "*.build.id": true}"#);
    let Value::Object(ref mut json) = value else {
        panic!("expected an object");
    };
    let mut settings = Settings::default();

    let props = apply_settings(json, &mut settings);

    assert_eq!(
        props.get("*.security_patch").map(String::as_str),
        Some("20180805")
    );
    assert_eq!(props.get("*.build.id").map(String::as_str), Some("true"));
    assert!(json.is_empty());
}
