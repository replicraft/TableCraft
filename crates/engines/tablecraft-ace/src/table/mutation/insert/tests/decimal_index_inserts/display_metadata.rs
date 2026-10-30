//! Numeric display places are independent of the physical Decimal domain.
mod api;

use super::super::passive_defaults::model as properties;
use super::{model, original};
use model::{PAGE, Row, Settings};
use properties::{Property, field};
use std::io::Cursor;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_model::CatalogScope;

fn source(code: u8, places: u8) -> model::Case {
    let mut settings = Settings::ordinary(false, false);
    settings.scale = 28;
    let mut case = model::source(code, settings, false);
    properties::properties(
        &mut case.before,
        &[field(
            "Score",
            vec![
                Property {
                    flags: 1,
                    ..Property::text("Format", "0.00")
                },
                Property {
                    name: "DecimalPlaces",
                    kind: 2,
                    flags: 1,
                    data: vec![places],
                },
            ],
        )],
    );
    case
}

fn additions() -> [Row; 2] {
    [
        Row {
            id: 4,
            amount: Some(-12345),
            guid: None,
        },
        Row {
            id: 5,
            amount: None,
            guid: None,
        },
    ]
}

fn checked(bytes: &[u8], rows: usize) {
    original::checked(bytes, rows, 4);
    let report = crate::check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(
        report.table_validation(),
        crate::RuleCheckStatus::Unsupported
    );
    assert!(report.fields().iter().all(|field| {
        ![field.required(), field.empty_text(), field.validation()]
            .contains(&crate::RuleCheckStatus::Unsupported)
    }));
}

#[test]
fn qualify_passive_display_decimal_sources_and_full_scale28_expected_before_api() {
    for code in [5, 6] {
        for places in [0, 15, 255] {
            let case = source(code, places);
            let rows = additions();
            let expected = model::expected(&case, &rows);
            checked(&case.before, 3);
            checked(&expected, 5);
            assert_eq!(
                &expected[4 * PAGE..5 * PAGE],
                &case.before[4 * PAGE..5 * PAGE]
            );
            let snapshot = crate::read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                snapshot.rows()[3][1],
                Value::Decimal(Decimal::new(-12345, 28).unwrap())
            );
            assert_eq!(snapshot.rows()[4][1], Value::Null);
        }
    }
}
