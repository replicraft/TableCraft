//! Public INSERT candidates checked against independent signed-key whole images.
use super::model as base;
use crate::{RowInsert, inspect_table_index_counter, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::insert_seed as source_base;

pub(super) mod model;

fn checked(bytes: &[u8], expected: &[Vec<Value>], aliases: bool, counter: u32) {
    base::checked(bytes, expected.len(), if aliases { 2 } else { 1 });
    let actual = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut base::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual.rows().len(), expected.len());
    for (ordinal, (actual, expected)) in actual.rows().iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "typed stored row {ordinal}");
    }
    let mut physical = None;
    for name in if aliases {
        vec!["PrimaryID", "PrimaryAlias"]
    } else {
        vec!["PrimaryID"]
    } {
        let count = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            name,
            &mut base::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(count.value(), counter);
        assert_eq!(count.physical_index(), 0);
        if let Some(range) = &physical {
            assert_eq!(&count.byte_range(), range);
        } else {
            physical = Some(count.byte_range());
        }
    }
}

fn exercise(full: bool) {
    for code in [5, 6] {
        for aliases in [false, true] {
            let source = model::source(code, aliases, full);
            checked(&source.bytes, &source.rows, aliases, 451);
            let values = model::new_values(full);
            let mut final_rows = source.rows.clone();
            final_rows.push(values.clone());
            let (expected, reference) = model::expected(&source);
            checked(&expected, &final_rows, aliases, 452);
            assert_eq!(reference, if full { 26 << 8 } else { (15 << 8) | 196 });
            let requests = [RowInsert::new(values)];
            let candidate = base::capture(&source.bytes)
                .preview_ace16_insert_rows("Items", &requests, &mut base::budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.changed_rows(), 1);
            model::assert_image(candidate.candidate_bytes(), &expected);
            checked(candidate.candidate_bytes(), &final_rows, aliases, 452);
        }
    }
}

#[test]
fn fitted_data_insert_splits451_signed_primary_entries_once_across_aliases() {
    exercise(false);
}

#[test]
fn full_data_insert_composes_data26_right27_left28_and_shared_global_bits() {
    exercise(true);
}

#[test]
fn a_healthy_split_result_accepts_a_fitted_followup_without_changing_its_root() {
    for code in [5, 6] {
        let source = model::source(code, true, false);
        checked(&source.bytes, &source.rows, true, 451);
        let values = model::new_values(false);
        let mut rows = source.rows.clone();
        rows.push(values.clone());
        let (expected, _) = model::expected(&source);
        checked(&expected, &rows, true, 452);
        let candidate = base::capture(&source.bytes)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(values)],
                &mut base::budget(),
                &|| false,
            )
            .unwrap();
        model::assert_image(candidate.candidate_bytes(), &expected);
        checked(candidate.candidate_bytes(), &rows, true, 452);
        let followup_values = vec![Value::Int32(227), Value::Binary(Vec::new())];
        let followup = base::capture(candidate.candidate_bytes())
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(followup_values.clone())],
                &mut base::budget(),
                &|| false,
            )
            .unwrap();
        rows.push(followup_values);
        checked(followup.candidate_bytes(), &rows, true, 453);
        assert_eq!(
            followup.candidate_bytes().len(),
            candidate.candidate_bytes().len()
        );
        assert!(
            followup.candidate_bytes()[13 * source_base::PAGE..14 * source_base::PAGE]
                == candidate.candidate_bytes()[13 * source_base::PAGE..14 * source_base::PAGE]
        );
        model::assert_image(candidate.candidate_bytes(), &expected);
    }
}
