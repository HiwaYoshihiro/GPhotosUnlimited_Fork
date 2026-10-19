//! Comment stripping and property dispatch.
//!
//! These cover the paths that run inside Google Photos on every config read and
//! every property lookup, so they are the ones where a bug is both silent and
//! expensive.

use core::ffi::{c_char, c_void};
use std::ffi::{CStr, CString};

use serde_json::Value;
use unlimitedphotos::config::{parse_config, parse_int};
use unlimitedphotos::props::{modify_callback, resolve, CallContext, PropertyCallback};

#[test]
fn json_comments_are_stripped_around_unicode_values() {
    let text = r#"{
        // a comment
        "MODEL": "日本語のモデル",
        /* another comment */
        "emoji": "📷🚀",
        "accent": "café naïve",
        "PIXEL": "true"
    }"#;

    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(
        map.get("MODEL"),
        Some(&Value::String("日本語のモデル".into()))
    );
    assert_eq!(map.get("emoji"), Some(&Value::String("📷🚀".into())));
    assert_eq!(map.get("accent"), Some(&Value::String("café naïve".into())));
    assert_eq!(map.len(), 4);
}

#[test]
fn comments_inside_strings_are_preserved() {
    // A `//` inside a value is not a comment, and stripping it would truncate
    // the spoofed property.
    let text = r#"{"url": "https://example.com/a", "note": "a /* b */ c"}"#;

    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(
        map.get("url"),
        Some(&Value::String("https://example.com/a".into()))
    );
    assert_eq!(map.get("note"), Some(&Value::String("a /* b */ c".into())));
}

#[test]
fn escapes_do_not_end_a_string_early() {
    // The escaped quote must not close the string, otherwise the `//` that
    // follows would be treated as a comment and the document would be
    // truncated.
    let text = r#"{"a": "say \"hi\" // still a value", "b": "ok"}"#;

    let Value::Object(map) = parse_config(text) else {
        panic!("expected an object");
    };
    assert_eq!(
        map.get("a"),
        Some(&Value::String("say \"hi\" // still a value".into()))
    );
    assert_eq!(map.get("b"), Some(&Value::String("ok".into())));
}

#[test]
fn unicode_values_survive_the_prop_format() {
    let Value::Object(map) = parse_config("MODEL=日本語\nACCENT=café\n") else {
        panic!("expected an object");
    };
    assert_eq!(map.get("MODEL"), Some(&Value::String("日本語".into())));
    assert_eq!(map.get("ACCENT"), Some(&Value::String("café".into())));
}

#[test]
fn parse_int_boundaries_are_reported_not_truncated() {
    assert_eq!(parse_int("2147483647"), Some(i32::MAX));
    assert_eq!(parse_int("2147483648"), None);
    assert_eq!(parse_int("-2147483648"), Some(i32::MIN));
    assert_eq!(parse_int("-2147483649"), None);
}

/// Records what `modify_callback` forwarded.
struct Recorder {
    calls: usize,
    cookie: *mut c_void,
    value: Option<CString>,
}

unsafe extern "C" fn record(
    cookie: *mut c_void,
    _name: *const c_char,
    value: *const c_char,
    _serial: u32,
) {
    let recorder = &mut *(cookie as *mut Recorder);
    recorder.calls += 1;
    recorder.value = Some(CStr::from_ptr(value).to_owned());
}

fn recorder() -> Box<Recorder> {
    Box::new(Recorder {
        calls: 0,
        cookie: core::ptr::null_mut(),
        value: None,
    })
}

#[test]
fn dispatch_reaches_the_callback_with_its_own_cookie() {
    let mut recorder = recorder();
    let recorder_ptr = &mut *recorder as *mut Recorder as *mut c_void;
    recorder.cookie = recorder_ptr;

    let mut context = CallContext::new(record as PropertyCallback, recorder_ptr);
    let name = c"ro.build.type";
    let value = c"userdebug";

    unsafe {
        modify_callback(context.as_void_ptr(), name.as_ptr(), value.as_ptr(), 1);
    }

    assert_eq!(recorder.calls, 1);
    // Nothing was installed in this test process, so the value passes through.
    assert_eq!(recorder.value.as_deref(), Some(c"userdebug"));
}

#[test]
fn nested_contexts_do_not_cross_talk() {
    // Two calls in flight at once: the inner one must reach its own callback
    // with its own cookie. A global cookie->callback map could not promise
    // this when a cookie value is reused.
    let mut outer = recorder();
    let outer_ptr = &mut *outer as *mut Recorder as *mut c_void;
    outer.cookie = outer_ptr;

    let mut inner = recorder();
    let inner_ptr = &mut *inner as *mut Recorder as *mut c_void;
    inner.cookie = inner_ptr;

    let mut outer_context = CallContext::new(record as PropertyCallback, outer_ptr);
    let mut inner_context = CallContext::new(record as PropertyCallback, inner_ptr);

    unsafe {
        modify_callback(
            inner_context.as_void_ptr(),
            c"ro.inner".as_ptr(),
            c"1".as_ptr(),
            0,
        );
        modify_callback(
            outer_context.as_void_ptr(),
            c"ro.outer".as_ptr(),
            c"2".as_ptr(),
            0,
        );
    }

    assert_eq!(inner.calls, 1);
    assert_eq!(outer.calls, 1);
    assert_eq!(inner.value.as_deref(), Some(c"1"));
    assert_eq!(outer.value.as_deref(), Some(c"2"));
}

#[test]
fn a_null_context_is_ignored() {
    // The real implementation must never see a null context, but dereferencing
    // one here would take the whole app down rather than dropping a property.
    unsafe {
        modify_callback(
            core::ptr::null_mut(),
            c"ro.build.type".as_ptr(),
            c"user".as_ptr(),
            0,
        );
    }
}

#[test]
fn resolve_prefers_exact_then_first_sorted_wildcard() {
    let mut props = std::collections::BTreeMap::new();
    props.insert("ro.build.type".to_string(), CString::new("exact").unwrap());
    props.insert("*build.type".to_string(), CString::new("wild").unwrap());
    props.insert("*type".to_string(), CString::new("generic").unwrap());

    let exact = resolve(&props, "ro.build.type").unwrap();
    assert_eq!(unsafe { CStr::from_ptr(exact) }.to_str(), Ok("exact"));

    // `*build.type` sorts before `*type`, and the first match wins.
    let wild = resolve(&props, "ro.product.build.type").unwrap();
    assert_eq!(unsafe { CStr::from_ptr(wild) }.to_str(), Ok("wild"));

    assert!(resolve(&props, "ro.unrelated").is_none());
}

#[test]
fn wildcard_needs_a_leading_star() {
    let mut props = std::collections::BTreeMap::new();
    // A key without `*` must never match as a suffix rule; only exact.
    props.insert("build.type".to_string(), CString::new("no-star").unwrap());

    assert!(resolve(&props, "ro.build.type").is_none());
}
