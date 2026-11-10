//! Populated metadata renames preserve independently authored row/key images.
use super::*;
use crate::{NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

const SCOPE: CatalogScope = CatalogScope::new(992);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first different offset: {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn nonempty_dense_batches_keep_payloads_nulls_keys_and_counters_at_every_name_width() {
    for values in [
        vec![Some(42)],
        vec![Some(42), None, Some(-7)],
        vec![None, Some(42)],
    ] {
        for name in ["TaxAmount", "X", "Total", "Score"] {
            let [before, expected] = fitting::populated_number_pair(name, &values);
            let facts = image(&before)
                .inspect_populated_field_rename_source("Table1", "Score", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(facts.identity().column_number(), 1);
            assert_eq!(facts.maximum_name_utf16_units(), 64);
            let candidate = image(&before)
                .prepare_populated_field_rename("Table1", "Score", name, &mut budget(), &|| false)
                .unwrap();
            equal_image(candidate.before_bytes(), &before);
            equal_image(candidate.after_bytes(), &expected);
            assert_eq!(candidate.changed(), name != "Score");
            assert_eq!(candidate.column(), "Score");
            assert_eq!(candidate.new_name(), name);
            assert_eq!(
                &candidate.after_bytes()[8 * 4096..10 * 4096],
                &before[8 * 4096..10 * 4096]
            );
            for offset in [16, 20, 67] {
                assert_eq!(
                    &candidate.after_bytes()[7 * 4096 + offset..7 * 4096 + offset + 4],
                    &(values.len() as u32).to_le_bytes()
                );
            }
            let reopened = image(candidate.after_bytes());
            let rows = read_table(
                &mut reopened.reader(),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected_rows = values
                .iter()
                .enumerate()
                .map(|(ordinal, value)| {
                    vec![
                        Value::Int32(ordinal as i32 + 1),
                        value.map_or(Value::Null, Value::Int32),
                    ]
                })
                .collect::<Vec<_>>();
            assert_eq!(rows.rows(), expected_rows);
        }
    }
}

#[test]
fn empty_profiles_and_populated_profile_never_retry_each_others_source_languages() {
    let [before, _] = fitting::populated_number_pair("TaxAmount", &[Some(42), None]);
    assert!(
        image(&before)
            .prepare_fitted_field_rename("Table1", "Score", "TaxAmount", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_same_width_field_rename("Table1", "Score", "Total", &mut budget(), &|| false)
            .is_err()
    );
    let empty = fitting::source(SourceCase::NumberFirst);
    assert!(matches!(
        image(&empty).prepare_populated_field_rename(
            "Table1",
            "Score",
            "TaxAmount",
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
    assert!(matches!(
        image(&before).prepare_populated_field_rename(
            "Table1",
            "ID",
            "Code",
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
    assert!(matches!(
        image(&before).prepare_populated_field_rename(
            "Table1",
            "Score",
            "ID",
            &mut budget(),
            &|| false
        ),
        Err(Error::DuplicateName)
    ));
    assert!(matches!(
        image(&before).prepare_populated_field_rename(
            "Table1",
            "Score",
            "雪",
            &mut budget(),
            &|| false
        ),
        Err(Error::PopulatedNameShape)
    ));
}

#[test]
fn actual_native_membership_and_complete_replay_reject_changed_rows_keys_and_inactive_history() {
    let [before, _] = fitting::populated_number_pair("TaxAmount", &[None, Some(42)]);
    for offset in [7 * 4096 + 20, 7 * 4096 + 67, 9 * 4096 + 482] {
        let mut forged = before.clone();
        forged[offset] ^= 1;
        assert!(
            image(&forged)
                .prepare_populated_field_rename(
                    "Table1",
                    "Score",
                    "TaxAmount",
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }

    let mut inactive_history = before;
    let row = u16::from_le_bytes(
        inactive_history[8 * 4096 + 14..8 * 4096 + 16]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(
        &inactive_history[8 * 4096 + row + 6..8 * 4096 + row + 10],
        &[0; 4]
    );
    inactive_history[8 * 4096 + row + 6] = 42;
    assert!(matches!(
        image(&inactive_history).prepare_populated_field_rename(
            "Table1",
            "Score",
            "TaxAmount",
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
}
