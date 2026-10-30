use super::AccessEngineeringDeleter;
use crate::engineering::{DeleteMode, editor::tests::fixture, sql_delete};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::DeletableTable;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub(in crate::engineering) fn source(code: u8) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes
}

pub(in crate::engineering) fn expected(before: &[u8], selected: &[usize]) -> Vec<u8> {
    let mut after = before.to_vec();
    after[10 * 4096 + 16..10 * 4096 + 20]
        .copy_from_slice(&(3_u32 - selected.len() as u32).to_le_bytes());
    for &row in selected {
        let offset = 12 * 4096 + 14 + row * 2;
        let original = u16::from_le_bytes(before[offset..offset + 2].try_into().unwrap());
        after[offset..offset + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
    }
    after
}

pub(in crate::engineering) fn qualify(bytes: &[u8], rows: usize) {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows);
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
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert!(constraints.relationships().is_empty());
}

#[test]
fn qualify_sql_delete_sources_and_whole_expected_images_before_preview() {
    for code in [5, 6] {
        let before = source(code);
        qualify(&before, 3);
        for selected in [vec![], vec![1], vec![0, 2], vec![0, 1, 2]] {
            qualify(&expected(&before, &selected), 3 - selected.len());
        }
    }
}

#[test]
fn sql_delete_matches_complete_literal_images_in_both_explicit_modes() {
    for code in [5, 6] {
        for mode in [DeleteMode::Restrict, DeleteMode::Cascading] {
            for (predicate, selected) in [
                ("FALSE", vec![]),
                ("Id=2", vec![1]),
                ("Id IN (1,3)", vec![0, 2]),
                ("TRUE", vec![0, 1, 2]),
            ] {
                let before = source(code);
                let after = expected(&before, &selected);
                qualify(&before, 3);
                qualify(&after, 3 - selected.len());
                let query = parse_query(
                    &format!("DELETE FROM Items WHERE {predicate}"),
                    ParseLimits::default(),
                )
                .unwrap();
                let plan = sql_delete::preview_delete(
                    capture(&before),
                    query.query(),
                    mode,
                    &mut budget(),
                    None,
                    &|| false,
                )
                .unwrap();
                assert_eq!(plan.matched_rows(), selected.len());
                let copy = plan.into_backend();
                assert_eq!(copy.affected_rows(), selected.len());
                assert_eq!(copy.before_bytes(), before);
                assert_eq!(copy.candidate_bytes(), after);
            }
        }
    }
}

#[test]
fn delete_rejects_cross_snapshot_handles_and_duplicate_original_handles() {
    let before = source(5);
    qualify(&before, 3);
    let first = AccessEngineeringDeleter::new(
        capture(&before),
        "Items",
        DeleteMode::Restrict,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let handle = first.snapshot().row_handle(0).unwrap();
    let other = AccessEngineeringDeleter::new(
        capture(&before),
        "Items",
        DeleteMode::Restrict,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        other
            .plan_deletes(&[handle], &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        first
            .plan_deletes(&[handle, handle], &mut budget(), &|| false)
            .is_err()
    );
}

mod functions;
mod relationships;
