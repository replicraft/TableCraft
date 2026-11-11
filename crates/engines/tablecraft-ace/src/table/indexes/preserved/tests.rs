//! Actual nullable native membership retains caller ordering and refusal controls.
use super::certify;
use crate::{EditError, InspectError, NativeImage, inspect_table_indexes};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

#[test]
fn independently_authored_index_accepts_root_orders_and_rejects_duplicate_or_missing_witnesses() {
    let case = raw::source(Population::NullableDuplicate, false);
    let scope = CatalogScope::new(28731);
    let mut budget = ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000);
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&case.expected),
        scope,
        &mut budget,
        &|| false,
    )
    .unwrap();
    let indexes =
        inspect_table_indexes(&mut image.reader(), scope, case.table, &mut budget, &|| {
            false
        })
        .unwrap();
    let index = indexes.indexes().last().unwrap();
    let mut expected = case
        .rows
        .iter()
        .map(|row| (row.reference, row.score))
        .collect::<Vec<_>>();
    assert_eq!(
        certify(
            &image,
            case.target_definition,
            index,
            &expected,
            &mut budget,
            &|| false
        )
        .unwrap(),
        case.index_pages
    );
    expected.reverse();
    assert_eq!(
        certify(
            &image,
            case.target_definition,
            index,
            &expected,
            &mut budget,
            &|| false
        )
        .unwrap(),
        case.index_pages
    );
    expected[0] = expected[1];
    assert!(matches!(
        certify(
            &image,
            case.target_definition,
            index,
            &expected,
            &mut budget,
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
    let mut expected = case
        .rows
        .iter()
        .map(|row| (row.reference, row.score))
        .collect::<Vec<_>>();
    expected[0].0 = 0;
    assert!(matches!(
        certify(
            &image,
            case.target_definition,
            index,
            &expected,
            &mut budget,
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
}
