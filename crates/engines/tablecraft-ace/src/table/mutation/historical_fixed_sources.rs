//! Independent whole-image source/expected gate before UPDATE admission.
use super::current_schema;
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1298);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn historical_score_update_sources_and_whole_expected_preserve_absent_bonus() {
    let [_, historical] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let mixed = populated::append_row(&historical, Some(11), Some(29));
    for before in [historical.as_slice(), mixed.as_slice()] {
        let expected = populated::update_scores(before, &[(0, Some(8)), (1, Some(12)), (2, None)]);
        for bytes in [before, expected.as_slice()] {
            let image =
                NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            current_schema::observe(&image, "Table1", &mut budget(), &|| false).unwrap();
        }
        let table = read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            &table.rows()[..3],
            &[
                vec![Value::Int32(1), Value::Int32(8), Value::Null],
                vec![Value::Int32(2), Value::Int32(12), Value::Null],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ]
        );
        if table.rows().len() == 4 {
            assert_eq!(
                table.rows()[3],
                vec![Value::Int32(4), Value::Int32(11), Value::Int32(29)]
            );
        }
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
        for (offset, (&old, &new)) in before.iter().zip(&expected).enumerate() {
            if old != new {
                assert!((8 * PAGE..9 * PAGE).contains(&offset));
            }
        }
    }
}
