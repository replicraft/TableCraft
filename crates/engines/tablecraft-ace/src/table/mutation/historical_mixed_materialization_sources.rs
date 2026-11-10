//! Independent mixed-row SOURCE/AFTER gate before any variable-prefix writer.
use super::current_schema;
use crate::{IndexCheckStatus, NativeImage, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_mixed::{self as raw, Case};

const SCOPE: CatalogScope = CatalogScope::new(1308);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn mixed_number_add_and_variable_footer_materialization_have_independent_complete_images() {
    for case in [Case::Text, Case::Binary, Case::LaterAbsentText] {
        let [original, before] = raw::pair(case);
        if !matches!(case, Case::LaterAbsentText) {
            let image =
                NativeImage::capture(&mut Cursor::new(&original), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let added = image
                .prepare_current_number_field_addition("Table1", "Bonus", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(added.after_bytes(), before);
        }
        let expected = raw::after(&before, 1, 37);
        for bytes in [before.as_slice(), expected.as_slice()] {
            let image =
                NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            current_schema::observe(&image, "Table1", &mut budget(), &|| false).unwrap();
            let checked = check_table(
                &mut Cursor::new(bytes),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 3);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
        }
        let table = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for (position, score) in [Value::Int32(42), Value::Null, Value::Int32(-7)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                table.rows()[position][0],
                Value::Int32((position + 1) as i32)
            );
            assert_eq!(table.rows()[position][2], score);
            assert_eq!(
                table.rows()[position][4],
                if position == 1 {
                    Value::Int32(37)
                } else {
                    Value::Null
                }
            );
            if matches!(case, Case::LaterAbsentText) {
                assert_eq!(table.rows()[position][5], Value::Null);
            }
        }
        assert_eq!(table.rows()[0][1], Value::Text("alpha".into()));
        assert_eq!(table.rows()[1][1], Value::Null);
        assert_eq!(table.rows()[2][1], Value::Text("wide".into()));
        if matches!(case, Case::Binary) {
            assert_eq!(table.rows()[0][3], Value::Binary(vec![0, 255, 1, 2]));
            assert_eq!(table.rows()[1][3], Value::Binary(vec![]));
        } else {
            assert_eq!(table.rows()[0][3], Value::Text("☃".into()));
            assert_eq!(table.rows()[1][3], Value::Text(String::new()));
        }
        assert_eq!(table.rows()[2][3], Value::Null);
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
    }
}
