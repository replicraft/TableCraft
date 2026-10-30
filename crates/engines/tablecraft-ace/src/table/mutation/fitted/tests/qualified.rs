//! Additive ownership-qualified derivatives of immutable historical fixtures.
use super::{budget, capture, model};
use crate::{
    IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints, read_table,
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use model::{Case, Family, Recipe};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

use super::framed::framed;

fn qualify(bytes: &[u8], rows: &[model::Row], family: Family) {
    let expected = rows
        .iter()
        .map(|row| model::values(row, family))
        .collect::<Vec<_>>();
    let read = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(read.rows(), expected);
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
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    assert!(report.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
}

fn pair(original: Case) {
    // Preserve read evidence for the exact old originals and expected images.
    qualify(&original.before, &original.rows, original.family);
    qualify(&original.expected, &original.final_rows, original.family);
    let derived = framed(original);
    for (bytes, rows) in [
        (&derived.before, &derived.rows),
        (&derived.expected, &derived.final_rows),
    ] {
        qualify(bytes, rows, derived.family);
        let image = capture(bytes);
        let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false).unwrap();
        Metadata::capture_for_fitted_updates(
            &image,
            checked.schema(),
            checked.indexes(),
            &[1, 2],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
fn qualify_framed_historical_fitting_derivatives_before_preview() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::Shorter, Recipe::Nulls] {
                    pair(model::case(code, family, descending, recipe, false));
                }
            }
        }
    }
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        pair(model::case(6, family, true, Recipe::Permutation, true));
    }
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            pair(crate::table::mutation::mixed_fitted_tests::capacity::case(
                family, descending,
            ));
        }
    }
}

fn refuses_historical_original(case: Case) {
    use super::{groups, requests};
    use crate::{EditError, WriteBlocker};

    let (fixed, variable) = requests(&case);
    let actual = capture(&case.before).preview_ace16_fitted_updates(
        "Items",
        &groups(&fixed, &variable),
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        actual,
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));

    // Empty selections still require complete source ownership metadata.
    let actual = capture(&case.before).preview_ace16_fitted_updates(
        "Items",
        &groups(&[], &[]),
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        actual,
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn historical_zero_tail_sources_remain_readable_but_refuse_fitting_mutation() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::Shorter, Recipe::Nulls] {
                    refuses_historical_original(model::case(
                        code, family, descending, recipe, false,
                    ));
                }
            }
        }
    }
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        refuses_historical_original(model::case(6, family, true, Recipe::Permutation, true));
    }
    for family in [Family::Legacy, Family::Modern] {
        for descending in [false, true] {
            refuses_historical_original(
                crate::table::mutation::mixed_fitted_tests::capacity::case(family, descending),
            );
        }
    }
}
