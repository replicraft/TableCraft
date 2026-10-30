//! SOURCE-only read gates over independently authored complete final images.
pub(in crate::table::mutation) mod capacity;
pub(in crate::table::mutation) mod growing;
pub(in crate::table::mutation) mod growing_boundary;
mod growing_roles;
mod key_changes;
pub(in crate::table::mutation) mod model;

use crate::test_support::base;
use crate::{
    IndexCheckStatus, RuleCheckStatus, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_schema, read_table,
};
use model::{Case, Family, PAGE, Recipe};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(bytes: &[u8], rows: &[model::Row], family: Family) {
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let declarations = schema
        .columns()
        .iter()
        .map(|field| {
            (
                field.name(),
                field.number(),
                field.flags(),
                field.extended_flags(),
                field.scalar_type(),
            )
        })
        .collect::<Vec<_>>();
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap_or_else(|error| panic!(
        "mixed source read failed: {error}; header={} family={family:?} declarations={declarations:?}",
        bytes[20],
    ));
    let expected = rows
        .iter()
        .map(|row| model::values(row, family))
        .collect::<Vec<_>>();
    assert_eq!(table.rows(), expected);

    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 4);
    assert_eq!(checked.indexes().len(), 6);
    for (position, index) in checked.indexes().iter().enumerate() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().data_number(), (position / 2) as u32);
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), [29, 31, 37][position / 2]);
    }

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
    assert!(report.relationships().is_empty());
}

fn preservation(case: &Case) {
    assert_eq!(case.before.len(), case.expected.len());
    assert_eq!(
        &case.before[10 * PAGE..11 * PAGE],
        &case.expected[10 * PAGE..11 * PAGE]
    );
    for page in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 14] {
        assert_eq!(
            &case.before[page * PAGE..(page + 1) * PAGE],
            &case.expected[page * PAGE..(page + 1) * PAGE]
        );
    }
    assert_eq!(case.records.len(), 4);
    assert_eq!(case.final_records.len(), 4);
    for (row, (before, after)) in case.records.iter().zip(&case.final_records).enumerate() {
        assert_eq!(before.len(), after.len());
        assert_eq!(&before[..6], &after[..6]);
        assert_eq!(before.last().unwrap() & !6, after.last().unwrap() & !6);
        if !case.changed.contains(&row) {
            assert_eq!(before, after);
        }
        if case.final_rows[row].age.is_none() {
            assert_eq!(&before[6..10], &after[6..10]);
        }
    }
    if case.forwarded {
        let slot = 12 * PAGE + 14;
        assert_eq!(&case.before[slot..slot + 2], &case.expected[slot..slot + 2]);
        let start = usize::from(
            u16::from_le_bytes(case.before[slot..slot + 2].try_into().unwrap()) & 0x1fff,
        );
        assert_eq!(
            &case.before[12 * PAGE + start..12 * PAGE + start + case.records[0].len()],
            &case.expected[12 * PAGE + start..12 * PAGE + start + case.records[0].len()]
        );
    }
}

#[test]
fn qualify_mixed_fitted_sources_and_complete_expected_before_preview() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::Shorter, Recipe::Nulls] {
                    let case = model::case(code, family, descending, recipe, false);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);
                    preservation(&case);
                }
            }
        }
    }
    for family in [Family::Legacy, Family::Modern, Family::Binary] {
        let case = model::case(6, family, true, Recipe::Permutation, true);
        qualify(&case.before, &case.rows, case.family);
        qualify(&case.expected, &case.final_rows, case.family);
        preservation(&case);
    }
}
