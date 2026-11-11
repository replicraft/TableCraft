//! Runtime and SQL share one positively selected ordinary retained INSERT family.
use super::{budget, test_support};
use crate::editing::{journal::NativeJournalInserter, offline::OfflineWriteSession};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, read_table};
use tablecraft_core::values::Value;
use tablecraft_db::{InsertValue, OmittingInsertableTable};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{field_changes, fields};

struct UnusedClock;

impl Clock for UnusedClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        panic!("ordinary keyless insertion must not sample a host clock")
    }
}

#[test]
fn reviewed_keyless_presence_saves_exact_original_family_and_reopens_without_generation() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let populated = field_changes::keyless_first_field(&seed, "Keyless");
    let before = field_changes::drop_field(&populated, definition, root, "Id");
    for (input, expected) in [
        (InsertValue::Omitted, None),
        (InsertValue::Supplied(Value::Null), None),
        (InsertValue::Supplied(Value::Int32(77)), Some(77)),
    ] {
        for with_clock in [false, true] {
            let after = field_changes::inserting::keyless_score(&before, definition, expected);
            let image = NativeImage::capture_retained(
                &mut Cursor::new(&before),
                CatalogScope::new(4631),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let inserter =
                NativeJournalInserter::new(image, "Keyless", &mut budget(), &|| false).unwrap();
            // Each review owns its original request; Value is an independent scalar carrier.
            let values = vec![match &input {
                InsertValue::Omitted => InsertValue::Omitted,
                InsertValue::Supplied(value) => InsertValue::Supplied(value.clone()),
            }];
            let plan = if with_clock {
                inserter.plan_insert_with_omissions_and_clock(
                    values,
                    &UnusedClock,
                    &mut budget(),
                    &|| false,
                )
            } else {
                inserter.plan_insert_with_omissions(values, &mut budget(), &|| false)
            }
            .unwrap();
            let original = plan.encode_intent(&mut budget(), &|| false).unwrap();
            assert!(original.starts_with(b"TCNA0010"));
            assert!(plan.requires_retained_capture());
            assert_eq!(plan.after_bytes(), after);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("keyless.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let staged =
                OfflineWriteSession::open(&path, CatalogScope::new(4632), &mut budget(), &|| false)
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
            let saved = fs::read(&path).unwrap();
            assert_eq!(saved, after);
            let table = read_table(
                &mut Cursor::new(saved),
                CatalogScope::new(4633),
                "Keyless",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                table.rows(),
                &[
                    vec![Value::Int32(23)],
                    vec![expected.map_or(Value::Null, Value::Int32)],
                ]
            );
        }
    }
}
