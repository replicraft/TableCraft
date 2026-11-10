//! Whole-image materialization oracle qualified before adding a writer.
use super::current_schema;
use crate::{IndexCheckStatus, NativeImage, check_table, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated::{
    self, materialization,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(1301);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

#[test]
fn qualify_independent_forwarded_bonus_materialization_without_default_backfill_or_index_rewrite() {
    let [_, ordinary] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let later = materialization::append_number(&ordinary, "Later");
    for mut before in [ordinary, later] {
        let first = usize::from(u16::from_le_bytes(
            before[8 * PAGE + 14..8 * PAGE + 16].try_into().unwrap(),
        ));
        // Bit3 is unused in the historical count2 mask. In the four-field
        // result it must become NULL for Later; bit7 must remain opaque.
        before[8 * PAGE + first + 10] |= 0x88;
        let expected = materialization::after(&before, 0, 2, 37);
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
        assert_eq!(table.rows().len(), 3);
        for (position, score) in [Value::Int32(42), Value::Null, Value::Int32(-7)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                table.rows()[position][0],
                Value::Int32((position + 1) as i32)
            );
            assert_eq!(table.rows()[position][1], score);
            assert_eq!(
                table.rows()[position][2],
                if position == 0 {
                    Value::Int32(37)
                } else {
                    Value::Null
                }
            );
            if table.columns().len() == 4 {
                assert_eq!(table.rows()[position][3], Value::Null);
            }
        }
        assert_eq!(&before[..8 * PAGE], &expected[..8 * PAGE]);
        assert_eq!(&before[9 * PAGE..], &expected[9 * PAGE..]);
        let target = usize::from(u16::from_le_bytes(
            expected[8 * PAGE + 20..8 * PAGE + 22].try_into().unwrap(),
        )) & 0x1fff;
        let fields = table.columns().len();
        let mask = expected[8 * PAGE + target + 2 + fields * 4];
        assert_eq!(mask & 0x80, 0x80);
        if fields == 4 {
            assert_eq!(mask & 8, 0);
        }
    }
}
