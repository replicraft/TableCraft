//! Full scalar source images qualified before any matching normal producer.
#[path = "wide_complete_insert/fixtures.rs"]
mod fixtures;
#[path = "wide_complete_insert/recovery.rs"]
mod recovery;
#[path = "wide_complete_insert/resources.rs"]
mod resources;

use super::{Storage, budget, resizable_storage::Target, scope};
use std::io::Cursor;
use tablecraft_ace::{
    NativeEofInsertIntent, NativeImage, NativeInsertIntent, RowInsert, read_table,
};
use tablecraft_core::values::{Decimal, Value};
use tablecraft_testkit::access::canonical_ordinary_wide_insert::Case;

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

#[test]
fn tcna4_complete_scalar_commits_match_all_sixteen_literal_images_and_rows() {
    let mut committed = 0;
    for view in fixtures::cases() {
        let plan = image(&view.source.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "Items",
                fixtures::requests(&view),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), view.source.before);
        assert_eq!(plan.after_bytes(), view.source.after);
        assert_eq!(plan.inserted_rows(), 1);
        let wire = fixtures::wire(&view);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire
        );
        assert!(NativeInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
        assert!(NativeEofInsertIntent::decode(&wire, &mut budget(), &|| false).is_err());
        assert_eq!(
            fixtures::intent(&view).rows()[0].values(),
            view.source.values
        );
        let mut target = Target::new(view.source.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut journal, &mut budget(), &|| false,)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.storage.bytes.get_ref(), &view.source.after);
        let expected_magic: &[u8] = if matches!(view.case, Case::EofFixed) {
            b"TCIMG002"
        } else {
            b"TCIMG001"
        };
        assert_eq!(&journal.bytes.get_ref()[..8], expected_magic);
        let rows = read_table(
            &mut Cursor::new(target.storage.bytes.get_ref()),
            scope(),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut expected = view.source.rows_before;
        expected.push(view.source.values);
        assert_eq!(rows.rows(), expected);
        committed += 1;
    }
    assert_eq!(committed, 16);
}

#[test]
fn complete_empty_and_wrong_carrier_scale_precision_preserve_old_closed_families() {
    for view in fixtures::cases() {
        let empty = image(&view.source.before)
            .plan_recoverable_ordinary_complete_row_inserts("Items", vec![], &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(empty.after_bytes(), view.source.before);
        assert_eq!(empty.inserted_rows(), 0);
        // Even NULL or empty old requests cannot open wider declarations.
        let mut nulls = vec![Value::Null; view.source.values.len()];
        let boolean = if matches!(
            view.case,
            Case::DecimalExact | Case::DecimalMax | Case::DecimalNull
        ) {
            4
        } else {
            0
        };
        // Boolean has a value bit, not a nullable scalar payload. Keep it
        // valid so a Boolean-NULL refusal cannot mask lost wide admission.
        nulls[boolean] = Value::Boolean(false);
        for rows in [vec![], vec![RowInsert::new(nulls)]] {
            assert!(
                image(&view.source.before)
                    .plan_recoverable_ordinary_row_inserts("Items", rows, &mut budget(), &|| false,)
                    .is_err()
            );
        }
        let mut wrong = view.source.values.clone();
        match view.case {
            Case::FixedExact => wrong[1] = Value::Int32(255),
            Case::DecimalExact => wrong[3] = Value::Decimal(Decimal::new(-12345, 3).unwrap()),
            Case::DecimalMax => {
                wrong[3] = Value::Decimal(Decimal::new(10_i128.pow(28), 4).unwrap())
            }
            _ => continue,
        }
        assert!(
            image(&view.source.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    "Items",
                    vec![RowInsert::new(wrong)],
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
}

#[path = "wide_complete_insert/journal_kind.rs"]
mod journal_kind;
