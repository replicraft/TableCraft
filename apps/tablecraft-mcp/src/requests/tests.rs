use super::validate;
use serde_json::json;

#[test]
fn rejects_fabricated_capability_fields_and_wrong_identity_types() {
    assert!(validate("apply_edit", &json!({"review_id": 7})).is_ok());
    assert!(validate("apply_edit", &json!({"review_id": "7"})).is_err());
    assert!(
        validate(
            "apply_edit",
            &json!({"review_id": 7, "before": [1, 2], "after": [3, 4]})
        )
        .is_err()
    );
    assert!(
        validate(
            "review_edit",
            &json!({"document_revision": 1, "draft_id": 3, "text": "42", "is_null": "false"})
        )
        .is_err()
    );
}

#[test]
fn validates_nested_field_declarations_and_native_widths_before_dispatch() {
    assert!(validate("review_table_creation", &json!({
        "document_revision": 1, "table": "T", "fields": [{"name": "Age", "kind": "number", "number_size": "byte"}]
    })).is_ok());
    assert!(validate("review_table_creation", &json!({
        "document_revision": 1, "table": "T", "fields": [{"name": "Age", "kind": "executable", "script": "run"}]
    })).is_err());
    assert!(validate("review_field_addition", &json!({
        "document_revision": 1, "table": "T", "column": "C", "kind": "short_text", "max_utf16_units": 256
    })).is_err());
}

#[test]
fn every_advertised_tool_has_a_derived_object_schema() {
    let tools = crate::tools::definitions().unwrap();
    assert!(tools.len() > 50);
    for tool in tools {
        assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
        assert_eq!(
            tool.input_schema["additionalProperties"], false,
            "{}",
            tool.name
        );
    }
}

#[test]
fn find_schemas_accept_explicit_modes_but_never_caller_authored_occurrences() {
    for matching in ["any", "whole", "start"] {
        for direction in ["all", "up", "down"] {
            assert!(
                validate(
                    "find_next",
                    &json!({
                        "document_revision": 1, "table": "Items", "column": "Score",
                        "pattern": "2", "matching": matching, "direction": direction,
                        "match_case": false,
                    })
                )
                .is_ok()
            );
        }
    }
    assert!(
        validate(
            "continue_find",
            &json!({"document_revision": 1, "find_id": 7})
        )
        .is_ok()
    );
    assert!(
        validate(
            "replace_found",
            &json!({
                "document_revision": 1, "find_id": 7, "replacement": "3",
            })
        )
        .is_ok()
    );
    for arguments in [
        json!({"document_revision": 1, "find_id": "7", "replacement": "3"}),
        json!({"document_revision": 1, "find_id": 7, "replacement": "3", "parent_ordinal": 99}),
        json!({"document_revision": 1, "find_id": 7, "replacement": "3", "span": {"start_utf16": 0, "end_utf16": 1}}),
    ] {
        assert!(validate("replace_found", &arguments).is_err());
    }
    assert!(
        validate(
            "find_next",
            &json!({
                "document_revision": 1, "table": "Items", "pattern": "2",
                "matching": "approximate", "direction": "all",
            })
        )
        .is_err()
    );
    assert!(
        validate(
            "find_next",
            &json!({
                "document_revision": 1, "table": "Items", "pattern": "2",
                "matching": "whole", "direction": "down", "formatted": true,
            })
        )
        .is_err()
    );
}
