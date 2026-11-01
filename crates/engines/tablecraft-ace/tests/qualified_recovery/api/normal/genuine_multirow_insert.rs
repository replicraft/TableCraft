//! Complete multirow candidates are compared only with independently read source images.
#[path = "genuine_multirow_insert/fixtures.rs"]
mod fixtures;
#[path = "genuine_multirow_insert/recovery.rs"]
mod recovery;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use std::io::Cursor;
use tablecraft_ace::{NativeImage, read_table};
use tablecraft_core::values::Value;

#[test]
#[ignore = "requires all independently qualified single-row and multirow INSERT source images"]
fn ordered_multirow_batches_commit_literal_images_and_retain_original_tcna_order() {
    let cases = fixtures::cases();
    let mut committed = 0;
    for case in cases {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let planned = image
            .plan_recoverable_ordinary_row_inserts(
                "Sentinel",
                fixtures::requests(case.reverse),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(planned.before_bytes(), case.before);
        assert_eq!(planned.after_bytes(), case.after);
        assert_eq!(planned.inserted_rows(), 2);
        let expected_rows = fixtures::rows(case.reverse);
        assert_eq!(planned.intent().table(), "Sentinel");
        assert_eq!(planned.intent().rows().len(), 2);
        for (request, expected) in planned.intent().rows().iter().zip(&expected_rows) {
            assert_eq!(request.values(), expected);
        }
        assert_eq!(
            planned.intent().encode(&mut budget(), &|| false).unwrap(),
            fixtures::wire(&case, case.reverse),
        );
        let decoded = fixtures::intent(&case);
        for (request, expected) in decoded.rows().iter().zip(&expected_rows) {
            assert_eq!(request.values(), expected);
        }
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            planned
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            2,
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed),
        );
        let table = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut expected = vec![vec![
            Value::Int32(23),
            Value::Text("unchanged sentinel".into()),
            Value::Binary(vec![199, 0, 255]),
        ]];
        expected.extend(expected_rows);
        assert_eq!(table.rows(), expected);
        committed += 1;
    }
    assert_eq!(committed, 4);
}
