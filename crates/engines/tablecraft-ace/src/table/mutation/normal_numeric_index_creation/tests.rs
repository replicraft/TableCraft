//! Candidate-only admission and preservation, independent of original languages.
mod matrix;
mod readonly_schema;
use crate::{EditError, NativeImage, SecondaryIndexCreate, WriteBlocker};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

const SCOPE: CatalogScope = CatalogScope::new(28701);

fn allowance(rows: usize) -> ResourceBudget {
    ResourceBudget::new(
        2 * 1024 * 1024 * 1024,
        rows.max(1) as u64 * 100 + 100_000,
        rows.max(1) as u64 * 40_000 + 16_000_000,
    )
}

fn image(bytes: &[u8], budget: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, budget, &|| false).unwrap()
}

fn same_bytes(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn shared_topology_keeps_all_existing_long_images_and_companions_exact() {
    for descending in [false, true] {
        for case in [
            raw::source(Population::Empty, descending),
            raw::source(Population::NullableDuplicate, descending),
            raw::source(Population::ThreeLeaves, descending),
            raw::protected_companion(descending),
        ] {
            let mut work = allowance(case.rows.len());
            let request = SecondaryIndexCreate::new(case.index_name, case.column, descending);
            let candidate = image(&case.before, &mut work)
                .prepare_retained_numeric_index_creation(case.table, &request, &mut work, &|| false)
                .unwrap();
            same_bytes(candidate.before_bytes(), &case.before);
            same_bytes(candidate.candidate_bytes(), &case.expected);
            assert_eq!(candidate.indexed_rows(), case.rows.len());
            assert_eq!(candidate.root_page(), case.root);
            assert_eq!(candidate.index_pages(), case.index_pages.len());
            assert_eq!(usize::from(candidate.height()) + 1, case.levels);
        }
    }
}

#[test]
fn source_authority_does_not_remove_existing_generated_indexed_or_namespace_guards() {
    let case = raw::protected_companion(false);
    let mut work = allowance(case.rows.len());
    assert!(matches!(
        image(&case.before, &mut work).prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new("OtherIndex", "ID", false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::Column))
    ));
    assert!(matches!(
        image(&case.expected, &mut work).prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new("OtherIndex", case.column, false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
    assert!(matches!(
        image(&case.before, &mut work).prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new("雪Index", case.column, false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    ));
    // A new ordinary numeric candidate does not widen either existing LONG
    // source language, even though Currency already has an exact key encoder.
    for fresh in [false, true] {
        let request = SecondaryIndexCreate::new("AmountIndex", "Amount", false);
        let original = image(&case.before, &mut work);
        let result = if fresh {
            original.prepare_retained_fresh_long_index_creation(
                case.table,
                &request,
                &mut work,
                &|| false,
            )
        } else {
            original
                .prepare_retained_long_index_creation(case.table, &request, &mut work, &|| false)
        };
        assert!(matches!(
            result,
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    let original = image(&case.before, &mut work);
    assert!(matches!(
        original.prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut work,
            &|| true,
        ),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    let original = image(&case.before, &mut work);
    assert!(matches!(
        original.prepare_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(EditError::Inspect(crate::InspectError::Limit(_)))
    ));
}

#[test]
fn independent_numeric_audit_rejects_wrong_identity_and_protected_byte_forgeries() {
    let case = raw::source(Population::NullableDuplicate, false);
    let mut work = allowance(case.rows.len());
    let before = image(&case.before, &mut work);
    let after = image(&case.expected, &mut work);
    assert!(matches!(
        super::super::index_population_audit::prove_numeric(
            &before,
            &after,
            case.table,
            &SecondaryIndexCreate::new("DifferentIndex", case.column, false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Preservation)
    ));
    let mut forged = case.expected;
    forged[4000] ^= 1;
    let after = image(&forged, &mut work);
    assert!(matches!(
        super::super::index_population_audit::prove_numeric(
            &before,
            &after,
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, false),
            &mut work,
            &|| false,
        ),
        Err(EditError::Preservation)
    ));
}
