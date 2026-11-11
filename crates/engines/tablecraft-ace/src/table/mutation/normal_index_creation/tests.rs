//! Independent literal images exercise complete source-to-candidate behavior.
use super::{NativeLongIndexCandidate, audit};
use crate::{EditError, NativeImage, SecondaryIndexCreate, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

const SCOPE: CatalogScope = CatalogScope::new(28361);

fn allowance(rows: usize) -> ResourceBudget {
    // Complete fresh source, producer, independent replay and all original-byte
    // comparisons share this explicit test allowance; this is not a format cap.
    ResourceBudget::new(
        2 * 1024 * 1024 * 1024,
        rows.max(1) as u64 * 100 + 100_000,
        rows.max(1) as u64 * 40_000 + 16_000_000,
    )
}

fn image(bytes: &[u8], budget: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, budget, &|| false).unwrap()
}

fn candidate(case: &raw::Source, budget: &mut ResourceBudget) -> NativeLongIndexCandidate {
    image(&case.before, budget)
        .prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
            budget,
            &|| false,
        )
        .unwrap_or_else(|error| {
            panic!(
                "rows={} descending={}: {error:?}",
                case.rows.len(),
                case.descending
            )
        })
}

fn same_bytes(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing byte offset"
    );
}

#[test]
fn nullable_directional_population_matches_independent_multilevel_images() {
    for population in [
        Population::Empty,
        Population::NullableDuplicate,
        Population::ThreeLeaves,
        Population::MultiLevel,
    ] {
        for descending in [false, true] {
            let case = raw::source(population, descending);
            let plan = candidate(&case, &mut allowance(case.rows.len()));
            same_bytes(plan.before_bytes(), &case.before);
            same_bytes(plan.candidate_bytes(), &case.expected);
            assert_eq!(plan.indexed_rows(), case.rows.len());
            assert_eq!(plan.root_page(), case.root);
            assert_eq!(plan.index_pages(), case.index_pages.len());
            assert_eq!(usize::from(plan.height()) + 1, case.levels);
            assert_eq!(plan.native_number(), 0);
            assert_eq!(plan.logical_index_number(), 0);
            assert_eq!(plan.physical_index_number(), 0);
            if matches!(population, Population::MultiLevel) {
                assert!(plan.height() >= 2);
            }
        }
    }
}

#[test]
fn generated_primary_currency_unicode_and_existing_storage_are_protected() {
    for descending in [false, true] {
        let case = raw::protected_companion(descending);
        let plan = candidate(&case, &mut allowance(case.rows.len()));
        same_bytes(plan.candidate_bytes(), &case.expected);
        assert_eq!(plan.native_number(), 1);
        assert_eq!(plan.logical_index_number(), 1);
        assert_eq!(plan.physical_index_number(), 1);
        assert_eq!(plan.table(), case.table);
        assert_eq!(plan.index_name(), case.index_name);
    }
}

#[test]
fn existing_indexes_wrong_field_conflicts_and_cancellation_do_not_create_candidates() {
    let case = raw::protected_companion(false);
    let mut budget = allowance(case.rows.len());
    let existing = image(&case.expected, &mut budget);
    assert!(matches!(
        existing.prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new("AnotherIndex", case.column, false),
            &mut budget,
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    let original = image(&case.before, &mut budget);
    assert!(matches!(
        original.prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new("PrimaryKey", case.column, false),
            &mut budget,
            &|| false
        ),
        Err(EditError::Conflict)
    ));
    let original = image(&case.before, &mut budget);
    assert!(matches!(
        original.prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, "Amount", false),
            &mut budget,
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    let original = image(&case.before, &mut budget);
    assert!(matches!(
        original.prepare_retained_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut budget,
            &|| true
        ),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
}

#[test]
fn independent_audit_refuses_wrong_original_request_and_unprotected_old_bytes() {
    let case = raw::source(Population::ThreeLeaves, false);
    let mut budget = allowance(case.rows.len());
    let before = image(&case.before, &mut budget);
    let after = image(&case.expected, &mut budget);
    let wrong_request = SecondaryIndexCreate::new("OtherName", case.column, false);
    assert!(matches!(
        audit::prove(
            &before,
            &after,
            case.table,
            &wrong_request,
            &mut budget,
            &|| false
        ),
        Err(EditError::Preservation)
    ));
    let mut forged = case.expected;
    // Inert header-page tail is outside every allowed index-creation edit.
    forged[4000] ^= 1;
    let after = image(&forged, &mut budget);
    let request = SecondaryIndexCreate::new(case.index_name, case.column, false);
    assert!(matches!(
        audit::prove(&before, &after, case.table, &request, &mut budget, &|| {
            false
        }),
        Err(EditError::Preservation)
    ));
}
