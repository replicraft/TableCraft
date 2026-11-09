use super::options;
use crate::requests::validate;
use serde_json::json;
use tablecraft_runtime::QueryRunOptions;

#[test]
fn absent_null_and_partial_options_keep_unspecified_runtime_defaults() {
    for arguments in [json!({}), json!({"options": null}), json!({"options": {}})] {
        assert_eq!(options(&arguments).unwrap(), QueryRunOptions::default());
    }
    let defaults = QueryRunOptions::default();
    let actual = options(&json!({
        "options": {"snapshot_rows": 200_000, "execution": {"steps": 100_000_000}}
    }))
    .unwrap();
    assert_eq!(actual.snapshot().maximum_rows(), 200_000);
    assert_eq!(
        actual.snapshot().batch_rows(),
        defaults.snapshot().batch_rows()
    );
    assert_eq!(actual.native(), defaults.native());
    assert_eq!(actual.query().bytes(), defaults.query().bytes());
    assert_eq!(actual.query().rows(), defaults.query().rows());
    assert_eq!(actual.query().steps(), 100_000_000);
    assert_eq!(actual.comparison(), defaults.comparison());
    assert_eq!(actual.presentation(), defaults.presentation());
}

#[test]
fn schema_validation_refuses_invalid_runtime_bounds_and_read_options_on_write_reviews() {
    for invalid in [
        json!({"snapshot_rows": 0}),
        json!({"snapshot_rows": 1_000_001}),
        json!({"batch_rows": 0}),
        json!({"native": {"rows": 0}}),
        json!({"execution": {"bytes": 0}}),
        json!({"execution": {"steps": 100_000_001}}),
        json!({"execution": {"bytes": 268_435_457}}),
        json!({"execution": {"rows": 1_000_001}}),
        json!({"comparison": {"rows": 1}}),
        json!({"presentation": {"rows": 1}}),
        json!({"execution": {"bytes": -1}}),
        json!({"execution": {"steps": "100"}}),
        json!({"execution": {"unlimited": true}}),
        json!({"source_path": "/unselected.accdb"}),
    ] {
        let arguments = json!({"document_revision": 1, "query_id": 2, "options": invalid});
        assert!(validate("run_query", &arguments).is_err());
        let saved = json!({"document_revision": 1, "object_id": "3", "name": "Saved", "options": arguments["options"]});
        assert!(validate("run_saved_query", &saved).is_err());
    }
    let arguments =
        json!({"document_revision": 1, "query_id": 2, "options": {"execution": {"steps": 100}}});
    assert!(validate("run_query", &arguments).is_ok());
    for operation in [
        "review_query_insert",
        "review_query_update",
        "review_query_delete",
    ] {
        assert!(validate(operation, &arguments).is_err());
    }
}
