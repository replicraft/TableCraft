//! Independent SOURCE/expected gates; not registered until root review.
mod model;
mod preview;

use crate::{
    IndexCheckStatus, RelationshipCheckStatus, RuleCheckStatus, check_table,
    check_table_constraints, read_table,
};
use model::{Case, PAGE};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture as base;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(bytes: &[u8], rows: &[Vec<Value>], variable: bool, selected_number: usize) {
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), rows);
    assert_eq!(
        table
            .schema()
            .columns()
            .iter()
            .map(|column| column.number())
            .collect::<Vec<_>>(),
        if variable {
            vec![0, 2, 4]
        } else {
            vec![0, selected_number]
        }
    );

    let selected_position = table
        .schema()
        .columns()
        .iter()
        .position(|column| column.number() == selected_number)
        .unwrap();
    assert_eq!(table.columns()[selected_position].name(), "Score");
    if variable && selected_number == 4 {
        assert_eq!(selected_position, 2);
    }

    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 2);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.index().columns()[0].number() == selected_number)
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
    for field in constraints.fields() {
        assert!(
            field.required() != RuleCheckStatus::Unsupported
                && field.empty_text() != RuleCheckStatus::Unsupported
                && field.validation() != RuleCheckStatus::Unsupported,
            "field={} Required={:?} AllowZeroLength={:?} Validation={:?} variable={} selected_native={} selected_source_nulls={}",
            field.name(),
            field.required(),
            field.empty_text(),
            field.validation(),
            variable,
            selected_number,
            rows.iter()
                .filter(|row| matches!(row[selected_position], Value::Null))
                .count(),
        );
    }
    assert!(
        constraints
            .relationships()
            .iter()
            .all(|relationship| relationship.status() == RelationshipCheckStatus::Checked)
    );
}

fn preserved(case: &Case) {
    assert_eq!(case.before.len(), case.expected.len());
    assert_eq!(
        &case.before[10 * PAGE..11 * PAGE],
        &case.expected[10 * PAGE..11 * PAGE]
    );
    for row in 0..3 {
        let slot = 12 * PAGE + 14 + row * 2;
        let start = usize::from(u16::from_le_bytes(
            case.before[slot..slot + 2].try_into().unwrap(),
        ));
        let end = if row == 0 {
            PAGE
        } else {
            usize::from(u16::from_le_bytes(
                case.before[slot - 2..slot].try_into().unwrap(),
            ))
        };
        let before = &case.before[12 * PAGE + start..12 * PAGE + end];
        let after = &case.expected[12 * PAGE + start..12 * PAGE + end];
        assert_eq!(&before[..10], &after[..10]);
        assert_eq!(
            before.last().unwrap() & !(1 << case.selected_number),
            after.last().unwrap() & !(1 << case.selected_number)
        );
        assert_eq!(&before[14..before.len() - 1], &after[14..after.len() - 1]);
    }
}

#[test]
fn qualify_historical_full_width_sources_and_whole_expected_before_updates() {
    for code in [5, 6] {
        for descending in [false, true] {
            for variable in [false, true] {
                for nullable in [false, true] {
                    for selected_number in [2, 4] {
                        let case =
                            model::case(code, descending, variable, nullable, selected_number);
                        qualify(
                            &case.before,
                            &case.rows,
                            case.variable,
                            case.selected_number,
                        );
                        qualify(
                            &case.expected,
                            &case.final_rows,
                            case.variable,
                            case.selected_number,
                        );
                        preserved(&case);
                    }
                }
            }
        }
    }
}
