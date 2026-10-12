use super::*;

#[test]
fn marker_request_refuses_snapshot_and_authority_selectors() {
    for key in [
        "state",
        "expected_revision",
        "scope",
        "url",
        "collection",
        "destination",
        "verified_research",
    ] {
        let mut value = serde_json::json!({"reply_id":"reply"});
        value[key] = serde_json::json!("forged");
        assert!(
            serde_json::from_value::<MarkerRequest>(value).is_err(),
            "accepted {key}"
        );
    }
}

#[test]
fn marker_request_contains_only_business_identifier() {
    let request = MarkerRequest {
        reply_id: "reply".into(),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"reply_id":"reply"})
    );
}
