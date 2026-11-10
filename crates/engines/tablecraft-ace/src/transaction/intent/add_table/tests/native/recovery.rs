//! Native append replay binds the observed cohort as well as the declaration.
use crate::{
    AddTableIntentError, AddTableSourceProfile, CreateColumn, FirstTableDefinition,
    NativeAddTableIntent, NativeImage, NativeRecoveryQualificationError,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::{self, JournalError, RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_retained_add_table::fields, canonical_wide_table_creation as raw,
    },
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

fn definition() -> FirstTableDefinition {
    let names: Vec<_> = (0..255).map(raw::field_name).collect();
    let zero = Value::Int32(0);
    let mut columns = vec![
        CreateColumn::new(&names[0], ScalarType::Int32)
            .with_primary_key("PrimaryKey")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
    ];
    columns.extend(names[1..].iter().map(|name| {
        CreateColumn::new(name, ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
    }));
    FirstTableDefinition::capture_native("More", &columns, &mut budget(), &|| false).unwrap()
}

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn native_append_replays_exact_mixed_and_retained_cohorts_through_v3() {
    for (before, profile) in [
        (raw::source().after, AddTableSourceProfile::Mixed),
        (
            fields::source(fields::Case::FirstNumber).bytes,
            AddTableSourceProfile::Retained,
        ),
    ] {
        let expected = raw::appended::source(&before, "More").after;
        for committed in [false, true] {
            let plan = image(&before, 7101)
                .plan_add_table_native(definition(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.source_profile(), profile);
            assert_eq!(plan.after_bytes(), expected);
            let intent_bytes = plan.encode_intent(&mut budget(), &|| false).unwrap();
            assert_eq!(&intent_bytes[..8], b"TCAT0007");
            assert_eq!(
                intent_bytes[14],
                u8::from(profile == AddTableSourceProfile::Retained)
            );
            let mut target = MemoryStorage::new(before.clone());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_native(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes = Cursor::new(expected.clone());
            }
            assert_eq!(&journal_file.bytes.get_ref()[..8], b"TCIMG003");
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified =
                NativeAddTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_native_recovery(
                        checked,
                        CatalogScope::new(7102),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
            let outcome = qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &expected } else { &before }
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            NativeAddTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
                .unwrap()
                .qualify_native_recovery(checked, CatalogScope::new(7103), &mut budget(), &|| false)
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

#[test]
fn native_append_old_preparation_and_cohort_forgery_never_authorize_target_writes() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let expected = raw::appended::source(&before, "More").after;
    let plan = image(&before, 7111)
        .plan_add_table_native(definition(), &mut budget(), &|| false)
        .unwrap();
    let intent_bytes = plan.encode_intent(&mut budget(), &|| false).unwrap();
    let mut target = MemoryStorage::new(before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::InvalidJournal)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeAddTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(7112), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::AddTableIntent(
            AddTableIntentError::UnsupportedProfile
        ))
    ));
    let mut wrong_cohort = intent_bytes;
    wrong_cohort[14] = 0;
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeAddTableIntent::decode_native(&wrong_cohort, &mut budget(), &|| false)
            .unwrap()
            .qualify_native_recovery(checked, CatalogScope::new(7113), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::AddTableIntent(
            AddTableIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);
}
