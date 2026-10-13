use super::*;

#[test]
fn request_defaults_enable_remix_with_a_budget() {
    let r: RemixRequest = serde_json::from_str(r#"{"conversation":[],"reply":"hi"}"#).unwrap();
    assert!(r.enabled && !r.deliver);
    assert_eq!(r.timeout_ms, 60_000);
    assert_eq!(r.region, None);
}

#[test]
fn member_tables_match_constants_and_names_are_distinct() {
    assert_eq!(REMIX_METHODS, &["Remix"]);
    assert_eq!(HOST_METHODS.len(), 6);
    assert!(HOST_METHODS.contains(&host_methods::DELIVER));
    assert_ne!(REMIX_INTERFACE, HOST_INTERFACE);
    assert_ne!(REMIX_PATH, HOST_PATH);
}

#[test]
fn version_binds_same_major_with_at_least_the_host_minor() {
    assert!(is_compatible(CONTRACT_VERSION));
    assert!(is_compatible((CONTRACT_VERSION.0, CONTRACT_VERSION.1 + 1)));
    assert!(!is_compatible((CONTRACT_VERSION.0 + 1, 0)));
}

#[test]
fn skip_reasons_and_failures_serialize_content_free() {
    assert_eq!(
        serde_json::to_string(&SkipReason::BelowThreshold).unwrap(),
        "\"below_threshold\""
    );
    assert_eq!(
        serde_json::to_string(&HostFailure::Timeout).unwrap(),
        r#"{"kind":"timeout"}"#
    );
}
