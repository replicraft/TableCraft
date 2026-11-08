//! TCNT6 reconstructs keyless/manual-key canonical initialization through one V2 session.
use super::{Storage, Target, budget, intent, pending, scope};
use std::io::Cursor;
use tablecraft_ace::{
    CreateColumn, FirstTableDefinition, FirstTablePlan, NativeImage,
    NativeRecoveryQualificationError, check_table, qualify_native_first_table_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, V2State, inspect_v2};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn plan(key: Option<ScalarType>) -> FirstTablePlan {
    let mut score = CreateColumn::new("Score", key.unwrap_or(ScalarType::Int32));
    if key.is_some() {
        score = score.with_primary_key("ActualKey").unwrap();
    }
    let columns = [CreateColumn::short_text("Name", 20).unwrap(), score];
    let definition =
        FirstTableDefinition::capture_manual("Manual", &columns, &mut budget(), &|| false).unwrap();
    NativeImage::capture(
        &mut Cursor::new(canonical_blank_creation::source()),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_first_table(definition, &mut budget(), &|| false)
    .unwrap()
}

#[test]
fn manual_and_keyless_first_table_commit_and_cold_recovery_preserve_actual_key_metadata() {
    let before = canonical_blank_creation::source();
    for key in [
        None,
        Some(ScalarType::Int32),
        Some(ScalarType::Int64),
        Some(ScalarType::Currency),
    ] {
        let planned = plan(key);
        let after = planned.after_bytes().to_vec();
        let raw = planned.encode_intent(&mut budget(), &|| false).unwrap();
        assert_eq!(&raw[..8], b"TCNT0006");
        assert_eq!(
            after.len(),
            if key.is_some() { 10 * 4096 } else { 9 * 4096 }
        );
        let checked = check_table(
            &mut Cursor::new(&after),
            scope(),
            "Manual",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.schema().columns().len(), 2);
        assert_eq!(checked.schema().columns()[0].name(), "Name");
        assert_eq!(checked.schema().columns()[1].name(), "Score");
        assert!(
            checked
                .schema()
                .columns()
                .iter()
                .all(|column| column.flags() & 4 == 0)
        );
        assert_eq!(checked.indexes().len(), usize::from(key.is_some()));
        if key.is_some() {
            let primary = checked.indexes()[0].index();
            assert_eq!(primary.name(), "ActualKey");
            assert_eq!(primary.columns().len(), 1);
            assert_eq!(primary.columns()[0].number(), 1);
        }
        for committed in [false, true] {
            let mut target = Target::new(before.clone());
            let mut journal = if committed {
                Storage::new(Vec::new())
            } else {
                pending(&before, &after)
            };
            if committed {
                plan(key)
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap();
                assert_eq!(target.storage.bytes.get_ref(), &after);
            }
            let mut cold = Storage::new(journal.bytes.get_ref().clone());
            let checked = inspect_v2(&mut cold, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_first_table_recovery(
                checked,
                intent(&raw),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified.state(),
                if committed {
                    V2State::Committed
                } else {
                    V2State::Pending
                }
            );
            assert_eq!(
                qualified
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.storage.bytes.get_ref(),
                if committed { &after } else { &before }
            );
        }
    }
}

#[test]
fn manual_first_table_recovery_rejects_changed_requests_or_complete_after_without_writes() {
    let before = canonical_blank_creation::source();
    for keyed in [false, true] {
        let plan = plan(keyed.then_some(ScalarType::Int32));
        let original = plan.encode_intent(&mut budget(), &|| false).unwrap();
        for changed_request in [false, true] {
            let mut raw = original.clone();
            let mut after = plan.after_bytes().to_vec();
            if changed_request {
                *raw.last_mut().unwrap() = b'x';
            } else {
                *after.last_mut().unwrap() ^= 1;
            }
            let mut journal = pending(&before, &after);
            let bytes = journal.bytes.get_ref().clone();
            let writes = journal.writes;
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_first_table_recovery(
                    checked,
                    intent(&raw),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, writes);
            assert_eq!(journal.bytes.get_ref(), &bytes);
        }
    }
}
