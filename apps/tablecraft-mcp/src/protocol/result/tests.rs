use super::encode_with_limit;
use serde_json::json;

#[test]
fn ordinary_values_preserve_null_and_boolean_semantics() {
    let result = encode_with_limit(Ok(json!({ "value": null, "flag": false })), 128);
    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content,
        Some(json!({ "value": null, "flag": false }))
    );
}

#[test]
fn escaping_and_error_payloads_share_the_same_complete_or_error_budget() {
    for result in [
        Ok(json!({ "value": "\"\\".repeat(128) })),
        Err("sensitive".repeat(128)),
    ] {
        let result = encode_with_limit(result, 128);
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.as_ref().unwrap()["kind"],
            "output_limit"
        );
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(!encoded.contains("sensitive"));
        assert!(encoded.len() < 512);
    }
}
