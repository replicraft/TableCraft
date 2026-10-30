#![cfg(feature = "ace16-write-preview")]

#[path = "ace16_encoded_split_contract/authority.rs"]
mod authority;
#[allow(dead_code)]
#[path = "ace16_encoded_delete_contract/model.rs"]
mod encoded_model;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "ace16_encoded_split_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;
#[path = "ace16_encoded_split_contract/values.rs"]
mod values;

use std::io::Cursor;
use tablecraft_ace::{
    Ace16WritePreview, EditError, IndexCheckStatus, SlotScalarUpdate, check_table,
    check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn preview(bytes: &[u8], requests: &[SlotScalarUpdate]) -> Result<Ace16WritePreview, EditError> {
    encoded_model::capture(bytes).preview_ace16_split_slot_updates(
        "Items",
        "Score",
        requests,
        &mut budget(),
        &|| false,
    )
}

fn checked(bytes: &[u8]) {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
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
    assert_eq!(constraints.rows(), rows.rows().len());
    assert!(constraints.relationships().is_empty());
    for index in report.indexes() {
        let ordered = read_table_index(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(ordered.rows().len(), rows.rows().len());
    }
}

fn accepted(case: &model::Case) {
    checked(&case.bytes);
    checked(&case.expected);
    let requests = [SlotScalarUpdate::new(
        case.row,
        case.old.clone(),
        case.new.clone(),
    )];
    let result = preview(&case.bytes, &requests).unwrap();
    assert_eq!(result.before_bytes(), case.bytes);
    assert_eq!(result.candidate_bytes(), case.expected);
    assert_eq!(result.changed_rows(), usize::from(case.old != case.new));
    checked(result.candidate_bytes());
}
