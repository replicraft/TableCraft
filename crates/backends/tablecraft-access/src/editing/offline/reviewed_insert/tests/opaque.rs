//! Real original-presence reviews select opaque INSERT9 before closed INSERT8.
use super::{budget, test_support};
use crate::editing::{NativeJournalInserter, offline::OfflineWriteSession};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, read_table};
use tablecraft_core::values::Value;
use tablecraft_db::{InsertValue, OmittingInsertableTable};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    opaque_rows as raw, rows::Input,
};

#[test]
fn opaque_presence_review_saves_exact_image_and_reopens_generated_rows() {
    for case in raw::CASES {
        let source = raw::source(case);
        let before = raw::after(&source.bytes, source.definition, source.roots[0], Some(123));
        for (input, supplied, expected) in [
            (Input::Omitted, InsertValue::Omitted, Value::Int32(0)),
            (Input::Null, InsertValue::Supplied(Value::Null), Value::Null),
            (
                Input::Number(-42),
                InsertValue::Supplied(Value::Int32(-42)),
                Value::Int32(-42),
            ),
        ] {
            let after = raw::append(&before, source.definition, input);
            let image = NativeImage::capture_retained(
                &mut Cursor::new(&before),
                CatalogScope::new(1698),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let plan = NativeJournalInserter::new(image, "Table1", &mut budget(), &|| false)
                .unwrap()
                .plan_insert_with_omissions(
                    vec![InsertValue::Omitted, supplied],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(
                plan.encode_intent(&mut budget(), &|| false)
                    .unwrap()
                    .starts_with(b"TCNA0009")
            );
            assert_eq!(plan.after_bytes(), after);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("opaque-insert.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let staged =
                OfflineWriteSession::open(&path, CatalogScope::new(1699), &mut budget(), &|| false)
                    .unwrap()
                    .stage_reviewed_insert_with_budgets(plan, &mut budget(), &mut budget(), &|| {
                        false
                    })
                    .unwrap();
            test_support::assert_staged_read_state(&path, &before);
            assert!(!bundle.exists());
            let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.matched_rows(), None);
            assert_eq!(receipt.changed_rows(), Some(1));
            assert_eq!(receipt.schema_change(), None);
            test_support::assert_completion(&receipt);
            owner.release().unwrap();
            assert_eq!(fs::read(&path).unwrap(), after);
            let rows = read_table(
                &mut Cursor::new(fs::read(&path).unwrap()),
                CatalogScope::new(1700),
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows(),
                &[
                    vec![Value::Int32(1), Value::Int32(123)],
                    vec![Value::Int32(2), Value::Null],
                    vec![Value::Int32(3), expected],
                ]
            );
        }
    }
}
