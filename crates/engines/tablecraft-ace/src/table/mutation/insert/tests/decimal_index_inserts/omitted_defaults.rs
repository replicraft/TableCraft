//! Exact literal Decimal defaults retain the independently declared p/s domain.
mod api;

use super::super::passive_defaults::model as properties;
use super::{model, original};
use model::{PAGE, Row, Settings};
use properties::{Property, field};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

fn source(code: u8, precision: u8, scale: u8, text: &str) -> model::Case {
    let mut settings = Settings::ordinary(false, false);
    settings.precision = precision;
    settings.scale = scale;
    let mut case = model::source(code, settings, false);
    properties::properties(
        &mut case.before,
        &[field("Score", vec![Property::text("DefaultValue", text)])],
    );
    case
}

#[test]
fn qualify_omitted_decimal_default_sources_and_complete_expected_before_resolver() {
    for code in [5, 6] {
        for (scale, text, coefficient) in [
            (4, "12.3400", 123400_i128),
            (28, "0.125", 1_250_000_000_000_000_000_000_000_000_i128),
        ] {
            let case = source(code, 28, scale, text);
            let row = Row {
                id: 4,
                amount: Some(coefficient),
                guid: None,
            };
            let expected = model::expected(&case, std::slice::from_ref(&row));
            for (bytes, rows) in [(&case.before, 3), (&expected, 4)] {
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
            assert_eq!(snapshot.rows()[3], row.values(case.settings));
        }
        for (precision, scale, text) in [(3, 2, "10.00"), (28, 4, "1.23456")] {
            let case = source(code, precision, scale, text);
            original::checked(&case.before, 3, 4);
        }
    }
}
