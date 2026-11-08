//! Mixed schema actions retain their exact source language through cold V2 recovery.
use super::*;
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::{
    canonical_creation, canonical_creation_primary, canonical_native17_creation,
};

fn manual_definition(key: Option<ScalarType>) -> FirstTableDefinition {
    let score = CreateColumn::new("Score", key.unwrap_or(ScalarType::Int32));
    let score = if key.is_some() {
        score.with_primary_key("ActualKey").unwrap()
    } else {
        score
    };
    FirstTableDefinition::capture_manual(
        "More",
        &[CreateColumn::short_text("Name", 20).unwrap(), score],
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn mixed_source(keyless: bool) -> (Vec<u8>, Vec<Vec<Value>>) {
    if keyless {
        let original = canonical_creation::source(canonical_creation::Case::One);
        let rows = original
            .rows
            .iter()
            .map(|row| row.iter().copied().map(Value::Int32).collect())
            .collect();
        (canonical_native17_creation::source(original.bytes), rows)
    } else {
        let original = canonical_creation_primary::source(canonical_creation_primary::Case::One);
        (original.bytes, original.rows)
    }
}

fn mixed_plan(before: &[u8], target: Option<Option<ScalarType>>) -> AddTablePlan {
    let definition = match target {
        Some(key) => manual_definition(key),
        None => definition(),
    };
    NativeImage::capture(&mut Cursor::new(before), scope(), &mut budget(), &|| false)
        .unwrap()
        .plan_add_table(definition, &mut budget(), &|| false)
        .unwrap()
}

#[test]
fn mixed_v5_manual_keyless_and_generated_targets_recover_only_the_exact_original_images() {
    let generated = canonical_add_table::source();
    let (keyless, keyless_rows) = mixed_source(true);
    let (manual, manual_rows) = mixed_source(false);
    let generated_rows = vec![generated.original_row];
    for (before, rows, target) in [
        (&generated.before, &generated_rows, Some(None)),
        (
            &generated.before,
            &generated_rows,
            Some(Some(ScalarType::Int32)),
        ),
        (
            &generated.before,
            &generated_rows,
            Some(Some(ScalarType::Int64)),
        ),
        (
            &generated.before,
            &generated_rows,
            Some(Some(ScalarType::Currency)),
        ),
        (&keyless, &keyless_rows, None),
        (&manual, &manual_rows, None),
    ] {
        let planned = mixed_plan(before, target);
        let raw = planned.encode_intent(&mut budget(), &|| false).unwrap();
        let after = planned.after_bytes().to_vec();
        assert_eq!(&raw[..8], b"TCAT0005");
        assert_eq!(&after[7 * 4096..before.len()], &before[7 * 4096..]);
        for committed in [false, true] {
            let mut target_storage = Target::new(before.clone());
            let mut journal = Storage::new(Vec::new());
            if !committed {
                target_storage.storage.fail_prefix = Some(before.len() + 1);
            }
            let result = mixed_plan(before, target)
                .prepare(&mut target_storage, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false);
            if committed {
                result.unwrap();
            } else {
                assert!(result.is_err());
                assert!(target_storage.storage.bytes.get_ref().len() > before.len());
            }
            let mut cold = Storage::new(journal.bytes.get_ref().clone());
            let qualified = qualify(&mut cold, &raw, &mut budget(), &|| false).unwrap();
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
                    .recover(&mut target_storage, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            let expected = if committed { &after } else { before };
            assert_eq!(target_storage.storage.bytes.get_ref(), expected);
            let actual = read_table(
                &mut Cursor::new(expected),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(actual.rows(), rows);
        }
    }
}

#[test]
fn mixed_v5_cannot_downgrade_manual_source_or_upgrade_a_legacy_native_action() {
    // The independently authored manual source has the same ten-page length
    // as a legacy generated source. Source roles, not length, decide replay.
    let (before, _) = mixed_source(false);
    assert_eq!(before.len(), 10 * 4096);
    let planned = mixed_plan(&before, None);
    let raw = planned.encode_intent(&mut budget(), &|| false).unwrap();
    let after = planned.after_bytes().to_vec();
    let mut downgraded = raw.clone();
    downgraded[..8].copy_from_slice(b"TCAT0001");
    downgraded[12..14].copy_from_slice(&1_u16.to_le_bytes());
    NativeAddTableIntent::decode(&downgraded, &mut budget(), &|| false).unwrap();
    let mut journal = pending(&before, &after);
    let bytes = journal.bytes.get_ref().clone();
    let writes = journal.writes;
    let error = qualify(&mut journal, &downgraded, &mut budget(), &|| false)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        NativeRecoveryQualificationError::AddTable(AddTableError::Source)
    ));
    assert_eq!(journal.writes, writes);
    assert_eq!(journal.bytes.get_ref(), &bytes);

    let legacy = canonical_add_table::source();
    let mut promoted = plan(&legacy.before)
        .encode_intent(&mut budget(), &|| false)
        .unwrap();
    promoted[..8].copy_from_slice(b"TCAT0005");
    promoted[12..14].copy_from_slice(&5_u16.to_le_bytes());
    let mut journal = pending(&legacy.before, &legacy.after);
    let bytes = journal.bytes.get_ref().clone();
    let writes = journal.writes;
    let error = qualify(&mut journal, &promoted, &mut budget(), &|| false)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        NativeRecoveryQualificationError::AddTableIntent(AddTableIntentError::UnsupportedProfile)
    ));
    assert_eq!(journal.writes, writes);
    assert_eq!(journal.bytes.get_ref(), &bytes);
}

#[test]
fn mixed_v5_recovery_refuses_complete_image_tampering_resources_and_cancellation() {
    let (before, _) = mixed_source(true);
    let planned = mixed_plan(&before, None);
    let raw = planned.encode_intent(&mut budget(), &|| false).unwrap();
    let after = planned.after_bytes().to_vec();
    for before_changed in [false, true] {
        let mut original = before.clone();
        let mut replacement = after.clone();
        if before_changed {
            original[8 * 4096 + 512] ^= 1;
        } else {
            replacement[8 * 4096 + 512] ^= 1;
        }
        let mut journal = pending(&original, &replacement);
        let bytes = journal.bytes.get_ref().clone();
        let writes = journal.writes;
        let error = qualify(&mut journal, &raw, &mut budget(), &|| false)
            .err()
            .unwrap();
        assert!(if before_changed {
            matches!(
                error,
                NativeRecoveryQualificationError::AddTableIntent(
                    AddTableIntentError::BeforeMismatch
                )
            )
        } else {
            matches!(error, NativeRecoveryQualificationError::ImageMismatch)
        });
        assert_eq!(journal.writes, writes);
        assert_eq!(journal.bytes.get_ref(), &bytes);
    }
    let bytes = pending(&before, &after).bytes.into_inner();
    for mut work in [
        ResourceBudget::new(0, 100_000, 10_000_000),
        ResourceBudget::new(10_000_000, 0, 10_000_000),
        ResourceBudget::new(10_000_000, 100_000, 0),
    ] {
        let mut journal = Storage::new(bytes.clone());
        assert!(qualify(&mut journal, &raw, &mut work, &|| false).is_err());
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &bytes);
    }
    let mut journal = Storage::new(bytes.clone());
    let error = qualify(&mut journal, &raw, &mut budget(), &|| true)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        NativeRecoveryQualificationError::Inspect(InspectError::Cancelled)
    ));
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &bytes);
}
