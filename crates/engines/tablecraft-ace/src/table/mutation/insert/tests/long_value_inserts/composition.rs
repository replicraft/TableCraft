//! Two column owners compose LVAL, fresh DATA and primary growth without repair.
pub(super) mod model;
use super::original;
use crate::RowInsert;

#[test]
fn composition_sources_and_expected_images_qualify_before_any_insert_call() {
    for code in [5, 6] {
        for pressure in [false, true] {
            let case = model::source_and_expected(code, pressure);
            model::checked(&case.before, &case.rows);
            model::checked(&case.expected, &model::final_rows(&case));
        }
    }
}

#[test]
fn multiple_lval_owners_compose_fresh_data_index_growth_and_inline_capacity_fallback() {
    for code in [5, 6] {
        for pressure in [false, true] {
            let case = model::source_and_expected(code, pressure);
            model::checked(&case.before, &case.rows);
            model::checked(&case.expected, &model::final_rows(&case));
            let requests = case
                .added
                .iter()
                .map(|row| RowInsert::new(row.values()))
                .collect::<Vec<_>>();
            let candidate = original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
                .unwrap();
            let actual = candidate.candidate_bytes();
            assert_eq!(actual.len(), case.expected.len());
            let difference = actual
                .iter()
                .zip(&case.expected)
                .position(|(actual, expected)| actual != expected);
            assert!(
                difference.is_none(),
                "first two-owner LVAL composition image difference: {difference:?}"
            );
            model::checked(actual, &model::final_rows(&case));
        }
    }
}
