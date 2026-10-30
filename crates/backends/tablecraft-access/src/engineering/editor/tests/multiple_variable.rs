//! Whole-row frontend evidence from immutable native literal fixture owners.
mod api;
use super::{fixture as base, metadata_default::joint_split::source_model as frames};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

mod matrix;
#[path = "../../../../../../engines/tablecraft-ace/src/table/mutation/fitted/multiple_variable_tests/model.rs"]
mod model;
#[path = "../../../../../../engines/tablecraft-ace/src/table/mutation/fitted/multiple_variable_tests/third_model.rs"]
mod third_model;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn qualify(bytes: &[u8], rows: &[model::Row], family: model::Family) {
    let read = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = rows
        .iter()
        .map(|row| model::values(row, family))
        .collect::<Vec<_>>();
    assert!(read.rows() == expected, "whole typed frontend SOURCE");
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), rows.len());
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
}

#[test]
fn qualify_multiple_variable_access_sources_and_expected_before_frontend_api() {
    let cases = matrix::cases();
    assert_eq!(cases.len(), 44);
    for case in cases {
        assert_eq!(case.records.len(), case.rows.len());
        assert_eq!(case.final_records.len(), case.final_rows.len());
        assert_eq!(
            case.expected.len() - case.before.len(),
            (case.data_pages.len() + case.index_pages.len()) * 4096,
        );
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
    }
}
