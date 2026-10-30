//! Causal fitting control for a reader-valid DATA/INDEX body-role overlap.
mod diagnostics;
mod model;

use super::growing::joint_split_tests::model as frames;
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, NativeImage, SlotScalarUpdate, WriteBlocker,
    read_table,
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
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

fn qualify(bytes: &[u8], rows: &[frames::Row], overlap: bool) {
    let read = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows().len(), 64);
    for (ordinal, (actual, expected)) in read.rows().iter().zip(rows).enumerate() {
        let values = vec![
            Value::Int32(expected.id),
            Value::Int32(expected.age),
            expected.name.as_ref().map_or(Value::Null, |name| {
                Value::Text(String::from_utf8(name.clone()).unwrap())
            }),
        ];
        assert!(actual == &values, "typed fitting row {ordinal}");
    }
    let image = capture(bytes);
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let metadata = Metadata::capture_for_mixed_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let protected = metadata.require_data_page(12, &mut budget(), &|| false);
    if overlap {
        assert_eq!(
            protected,
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    } else {
        protected.unwrap();
    }
}

#[test]
fn qualify_fitting_body_role_sources_before_preview() {
    let case = model::case();
    qualify(&case.before, &case.rows, false);
    qualify(&case.expected, &case.final_rows, false);
    qualify(&case.aliased, &case.rows, true);
    qualify(&case.aliased_expected, &case.final_rows, true);
    assert_eq!(case.before.len(), case.expected.len());
}

fn same_image(actual: &[u8], expected: &[u8]) {
    let first = actual.iter().zip(expected).position(|(a, b)| a != b);
    let count = actual.iter().zip(expected).filter(|(a, b)| a != b).count();
    assert!(
        actual == expected,
        "whole image lengths {}/{}, first {first:?}, differences {count}",
        actual.len(),
        expected.len()
    );
}

#[test]
fn fitting_refuses_reader_valid_index_owned_data_body() {
    let case = model::case();
    let fixed = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(99))];
    let variable = [SlotScalarUpdate::new(
        0,
        Value::Null,
        Value::Text("a".into()),
    )];
    let groups = [
        FittedColumnUpdates::Fixed {
            column: "Age",
            updates: &fixed,
        },
        FittedColumnUpdates::Variable {
            column: "Name",
            updates: &variable,
        },
    ];
    let healthy = capture(&case.before)
        .preview_ace16_fitted_updates("Items", &groups, &mut budget(), &|| false)
        .unwrap();
    same_image(healthy.candidate_bytes(), &case.expected);
    assert_eq!(healthy.changed_rows(), 1);

    match capture(&case.aliased).preview_ace16_fitted_updates(
        "Items",
        &groups,
        &mut budget(),
        &|| false,
    ) {
        Err(EditError::Unsupported(WriteBlocker::RowLayout)) => {}
        Err(error) => panic!("unexpected fitting refusal: {error}"),
        Ok(candidate) => {
            same_image(candidate.candidate_bytes(), &case.aliased_expected);
            panic!(
                "fitting accepted reader-valid DATA/INDEX body overlap; changed rows {}",
                candidate.changed_rows()
            );
        }
    }
}

#[test]
fn fitting_protection_grants_no_map_record_rewrite_authority() {
    use crate::{format::reader::row_range, table::mutation::map_authority::Owner};

    let case = model::case();
    let image = capture(&case.before);
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
    let fitted = Metadata::capture_for_fitted_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let growing = Metadata::capture_for_mixed_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut budget(),
        &|| false,
    )
    .unwrap();

    for (owner, page, slot) in [
        (Owner::GlobalFree, 1, 0),
        (Owner::OrdinaryOwned { definition: 10 }, 11, 0),
        (Owner::OrdinaryCandidate { definition: 10 }, 11, 1),
        (
            Owner::IndexOwned {
                definition: 10,
                physical: 0,
            },
            14,
            0,
        ),
        (
            Owner::IndexOwned {
                definition: 10,
                physical: 1,
            },
            14,
            1,
        ),
    ] {
        let page_start = page * 4096;
        let (_, start, end) = row_range(&case.before[page_start..page_start + 4096], slot).unwrap();
        let range = page_start + start..page_start + end;
        let reference = ((page as u32) << 8) | slot as u32;

        // The identical, exact original record belongs to the allocation
        // capability. Nonallocating protection deliberately cannot rewrite it.
        growing
            .require_record_rewrite(owner, reference, range.clone(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            fitted.require_record_rewrite(owner, reference, range, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}
