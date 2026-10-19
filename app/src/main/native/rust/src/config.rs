//! Config parsing: `fgp.prop` / `custom.fgp.prop` (key=value) and
//! `custom.fgp.json`, plus the "Advanced Settings" integers.
//!
//! The old code accepted JSON with `//` and `/* */` comments
//! (`nlohmann::json::accept(str, /*ignore_comments=*/true)`), and users do
//! have commented configs, so comments are stripped before handing the text to
//! `serde_json`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::logd;

/// Spoofing toggles, defaulted exactly like the C++ globals.
#[derive(Clone, Copy)]
pub struct Settings {
    pub verbose_logs: i32,
    pub spoof_build: i32,
    pub spoof_props: i32,
    pub spoof_provider: i32,
    pub spoof_signature: i32,
    pub spoof_features: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            verbose_logs: 0,
            spoof_build: 1,
            spoof_props: 0,
            spoof_provider: 0,
            spoof_signature: 0,
            spoof_features: 1,
        }
    }
}

/// Properties to spoof, keyed by name. `BTreeMap`, not `HashMap`: wildcard
/// matching walks this in sorted order and the first match wins, which is what
/// `std::map` gave us. `HashMap` would make the winner depend on the hasher.
pub type Props = BTreeMap<String, String>;

/// Parses an integer by hand.
///
/// `str::parse` would be fine here, but the C++ build used `-fno-exceptions`
/// and could not use `std::stoi`; keeping the same strict digit-only rules
/// means a value like `"1 "` or `"0x1"` is still rejected rather than being
/// interpreted differently.
pub fn parse_int(s: &str) -> Option<i32> {
    let bytes = s.as_bytes();
    let mut pos = 0;
    let mut negative = false;
    if let Some(&first) = bytes.first() {
        if first == b'-' || first == b'+' {
            negative = first == b'-';
            pos = 1;
        }
    }
    if pos >= bytes.len() {
        return None;
    }
    // Accumulate in i64 and truncate like the C++ `int` arithmetic did, so an
    // out-of-range value behaves the same instead of panicking.
    let mut value: i64 = 0;
    for &byte in &bytes[pos..] {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.wrapping_mul(10).wrapping_add(i64::from(byte - b'0'));
    }
    let value = if negative { -value } else { value };
    Some(value as i32)
}

/// Trims spaces and tabs only. A trailing space in a config value would
/// otherwise be spoofed verbatim into a `Build` field.
fn trim(s: &str) -> &str {
    s.trim_matches(|c| c == ' ' || c == '\t')
}

/// Converts the legacy `key=value` format into a JSON object.
///
/// The old code built a JSON *string* by concatenation and re-parsed it, so a
/// value containing a quote or backslash silently invalidated the whole config
/// and the module did nothing. Building the map directly is equivalent for
/// every well-formed config and drops that failure mode.
fn props_to_json(text: &str) -> Map<String, Value> {
    let mut map = Map::new();
    let text = text.replace('\r', "");
    // A final line with no trailing newline still counts, otherwise the last
    // entry of every config would be silently dropped.
    for line in text.split('\n') {
        let line = trim(line);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            logd!("Invalid prop entry, skipping");
            continue;
        };
        // An inline comment ends the value.
        let value = value.split_once('#').map_or(value, |(value, _)| value);
        map.insert(
            trim(name).to_string(),
            Value::String(trim(value).to_string()),
        );
    }
    map
}

/// Removes `//` and `/* */` comments while leaving string contents alone.
fn strip_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < bytes.len() {
        let byte = bytes[i];
        if in_string {
            out.push(byte as char);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                out.push('"');
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            _ => {
                // Copy whole UTF-8 sequences verbatim so multi-byte characters
                // in a property value survive.
                let ch = text[i..].chars().next().unwrap_or('\u{fffd}');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// Parses a config document: JSON first (comments allowed), then the legacy
/// prop format if it does not look like JSON.
pub fn parse_config(text: &str) -> Value {
    if let Ok(value) = serde_json::from_str::<Value>(&strip_comments(text)) {
        return value;
    }
    logd!("Converting config from prop format to JSON format");
    Value::Object(props_to_json(text))
}

/// Reads one integer "Advanced Settings" entry, if present, then drops it so
/// only `Build` field names and feature names are left for the Java side.
pub fn read_setting(
    json: &mut Map<String, Value>,
    name: &str,
    target: &mut i32,
    description: Option<&str>,
    verbose_logs: i32,
) {
    let Some(value) = json.get(name) else {
        return;
    };
    let parsed = match value {
        Value::String(text) if !text.is_empty() => parse_int(text),
        _ => None,
    };
    match parsed {
        Some(parsed) => {
            *target = parsed;
            if verbose_logs > 0 {
                if let Some(description) = description {
                    logd!(
                        "{} {}!",
                        description,
                        if parsed > 0 { "enabled" } else { "disabled" }
                    );
                }
            }
        }
        None => logd!("Error parsing {}!", name),
    }
    json.remove(name);
}

/// Applies the settings entries and splits property/feature entries out of the
/// document. `json` is left holding only the `Build` fields and features that
/// the Java side consumes.
pub fn apply_settings(json: &mut Map<String, Value>, settings: &mut Settings) -> Props {
    logd!("JSON contains {} keys!", json.len());

    let mut verbose = settings.verbose_logs;
    read_setting(
        json,
        "verboseLogs",
        &mut verbose,
        None,
        settings.verbose_logs,
    );
    settings.verbose_logs = verbose;
    if settings.verbose_logs > 0 {
        logd!("Verbose logging (level {}) enabled!", settings.verbose_logs);
    }

    read_setting(
        json,
        "spoofBuild",
        &mut settings.spoof_build,
        Some("Spoofing Build Fields"),
        settings.verbose_logs,
    );
    read_setting(
        json,
        "spoofProps",
        &mut settings.spoof_props,
        Some("Spoofing System Properties"),
        settings.verbose_logs,
    );
    read_setting(
        json,
        "spoofProvider",
        &mut settings.spoof_provider,
        Some("Spoofing Keystore Provider"),
        settings.verbose_logs,
    );
    read_setting(
        json,
        "spoofSignature",
        &mut settings.spoof_signature,
        Some("Spoofing ROM Signature"),
        settings.verbose_logs,
    );
    read_setting(
        json,
        "spoofFeatures",
        &mut settings.spoof_features,
        Some("Spoofing Pixel System Features"),
        settings.verbose_logs,
    );

    let mut props = Props::new();
    let mut erase = Vec::new();
    for (key, value) in json.iter() {
        if settings.verbose_logs > 1 {
            logd!("Parsing {}", key);
        }
        if !key.contains(['*', '.']) {
            continue;
        }
        // Property names are lower-case (e.g. "*.security_patch"); Pixel
        // feature markers have an upper-case leaf segment (e.g.
        // "...feature.PIXEL_EXPERIENCE"). Features are handled Java-side, so
        // just leave those in json as-is.
        let is_feature = key.bytes().any(|byte| byte.is_ascii_uppercase());
        if is_feature {
            if !matches!(value, Value::String(text) if !text.is_empty()) {
                logd!("{} is empty, skipping", key);
                erase.push(key.clone());
            }
            continue;
        }

        match value {
            Value::String(text) => {
                if text.is_empty() {
                    logd!("{} is empty, skipping", key);
                } else {
                    if settings.verbose_logs > 0 {
                        logd!("Adding '{}' to properties list", key);
                    }
                    props.insert(key.clone(), text.clone());
                }
            }
            _ => logd!("Error parsing {}!", key),
        }
        erase.push(key.clone());
    }

    for key in erase {
        json.remove(&key);
    }
    props
}
