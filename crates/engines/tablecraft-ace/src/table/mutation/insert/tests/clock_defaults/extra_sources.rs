//! Additional literal presence cases are qualified before clock capability calls.
use super::{checked, model, original};
use crate::read_table;
use model::{PAGE, Row};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

pub(super) struct Example {
    pub(super) source: model::Case,
    pub(super) row: Row,
    pub(super) reads_clock: bool,
}

pub(super) fn examples(code: u8, generated: bool) -> [Example; 2] {
    let id = if generated { 261 } else { 10 };
    [
        Example {
            source: model::source(code, generated, "Date()", "Now()"),
            row: Row {
                dated: Some(0x40e1dd2000000000),
                id,
                stamped: None,
            },
            reads_clock: true,
        },
        Example {
            source: model::source(code, generated, "NULL", "Null"),
            row: Row {
                dated: None,
                id,
                stamped: None,
            },
            reads_clock: false,
        },
    ]
}

#[test]
fn qualify_clock_default_mixed_presence_and_literal_null_before_callback() {
    for code in [5, 6] {
        for generated in [false, true] {
            for example in examples(code, generated) {
                checked(&example.source.before, 2);
                let expected = model::expected(&example.source, std::slice::from_ref(&example.row));
                checked(&expected, 3);
                assert_eq!(
                    &expected[4 * PAGE..5 * PAGE],
                    &example.source.before[4 * PAGE..5 * PAGE]
                );
                let snapshot = read_table(
                    &mut Cursor::new(&expected),
                    CatalogScope::new(1),
                    "Items",
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(snapshot.rows()[2], example.row.values());
            }
        }
    }
}
