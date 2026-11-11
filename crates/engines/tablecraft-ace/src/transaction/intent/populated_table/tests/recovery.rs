//! Cold V3 reconstruction compares independent complete table/row images.
use super::{budget, declaration};
use crate::{
    CreateColumn, FirstTableDefinition, InspectError, NativeImage, NativePopulatedAddTableIntent,
    NativeRecoveryQualificationError, PopulatedTableIntentError,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Value},
};
use tablecraft_io::journal::{self, JournalError, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_schema::DecimalConstraints;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{fields, populated as raw},
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(19006);

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn intent(before: &[u8], values: &[Option<i32>]) -> Vec<u8> {
    let rows = values
        .iter()
        .map(|value| vec![value.map_or(Value::Null, Value::Int32)])
        .collect();
    NativePopulatedAddTableIntent::new(declaration(), rows, before, &mut budget(), &|| false)
        .unwrap()
        .encode(&mut budget(), &|| false)
        .unwrap()
}

fn pending(before: &[u8], after: &[u8]) -> (MemoryStorage, MemoryStorage) {
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    (target, journal)
}

#[test]
fn populated_pending_committed_and_settled_sessions_reconstruct_every_original_row() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let literals = (0..900)
        .map(|value| (value % 17 != 0).then_some(value))
        .collect::<Vec<_>>();
    let expected = raw::numbers(&before, "Archive", &literals);
    for committed in [false, true] {
        let values = literals
            .iter()
            .map(|value| vec![value.map_or(Value::Null, Value::Int32)])
            .collect();
        let plan = image(&before)
            .plan_populated_add_table(declaration(), values, &mut budget(), &|| false)
            .unwrap();
        assert_image(plan.after_bytes(), &expected);
        let encoded = plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert_eq!(&encoded[..8], b"TCPT0001");
        let mut target = MemoryStorage::new(before.clone());
        let mut journal_file = MemoryStorage::new(Vec::new());
        let prepared = plan
            .prepare_populated(&mut target, &mut journal_file, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            let mut mixed = expected.clone();
            let split = before.len() / 2;
            mixed[..split].copy_from_slice(&before[..split]);
            target.bytes = Cursor::new(mixed);
        }
        assert_eq!(&journal_file.bytes.get_ref()[..8], b"TCIMG003");
        assert!(matches!(
            journal::inspect_v2(&mut journal_file, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let qualified = NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            qualified.state(),
            if committed {
                ResizableState::Committed
            } else {
                ResizableState::Pending
            }
        );
        let writes = target.writes;
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
        assert_image(
            target.bytes.get_ref(),
            if committed { &expected } else { &before },
        );
        if committed {
            assert_eq!(target.writes, writes);
        }
        let writes = target.writes;
        let journal_writes = journal_file.writes;
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let qualified = NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            outcome
        );
        assert_eq!(
            (target.writes, journal_file.writes),
            (writes, journal_writes)
        );
    }
}

#[test]
fn cold_variable_rows_use_actual_native_width_and_decimal_scale_without_sql_or_defaults() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let literals = [
        (Some("日本🦀"), Some(90_071_992_547_409_931_234)),
        (Some(""), None),
        (None, Some(-90_071_992_547_409_934_321)),
    ];
    let expected = raw::text_decimals(&before, "Archive", &literals);
    let columns = [
        CreateColumn::short_text("Label", 7).unwrap(),
        CreateColumn::decimal("Amount", DecimalConstraints::new(28, 4).unwrap()),
    ];
    let definition =
        FirstTableDefinition::capture_manual_native("Archive", &columns, &mut budget(), &|| false)
            .unwrap();
    let values = literals
        .iter()
        .map(|(text, decimal)| {
            vec![
                text.map_or(Value::Null, |text| Value::Text(text.into())),
                decimal.map_or(Value::Null, |coefficient| {
                    Value::Decimal(Decimal::new(coefficient, 4).unwrap())
                }),
            ]
        })
        .collect();
    let encoded =
        NativePopulatedAddTableIntent::new(definition, values, &before, &mut budget(), &|| false)
            .unwrap()
            .encode(&mut budget(), &|| false)
            .unwrap();
    // The cold journal's AFTER is the independent raw image, never producer output.
    let (mut target, mut journal_file) = pending(&before, &expected);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
        .unwrap()
        .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false)
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(target.writes, 0);
    assert_image(target.bytes.get_ref(), &before);
}

#[test]
fn wrong_before_after_values_and_split_pool_failures_never_gain_recovery_authority() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let expected = raw::numbers(&before, "Archive", &[Some(123), None]);
    let encoded = intent(&before, &[Some(123), None]);
    let (target, mut journal_file) = pending(&before, &expected);
    let journal_bytes = journal_file.bytes.get_ref().clone();
    let journal_writes = journal_file.writes;
    let mut stale = before.clone();
    let last = stale.len() - 1;
    stale[last] ^= 1;
    let stale = intent(&stale, &[Some(123), None]);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&stale, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::PopulatedTableIntent(
            PopulatedTableIntentError::BeforeMismatch
        ))
    ));
    let wrong = intent(&before, &[Some(124), None]);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&wrong, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    let mut native = ResourceBudget::new(u64::MAX, u64::MAX, 123);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        matches!(NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false).unwrap().qualify_recovery(checked, SCOPE, &mut native, &mut ResourceBudget::new(u64::MAX, u64::MAX, 0), &|| false), Err(NativeRecoveryQualificationError::Inspect(InspectError::Limit(limit))) if limit.resource == Resource::Steps)
    );
    assert_eq!(native.remaining(Resource::Steps), 123);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| true),
        Err(NativeRecoveryQualificationError::Inspect(
            InspectError::Cancelled
        ))
    ));
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        matches!(NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false).unwrap().qualify_recovery(checked, SCOPE, &mut ResourceBudget::new(u64::MAX, u64::MAX, 0), &mut budget(), &|| false), Err(NativeRecoveryQualificationError::Inspect(InspectError::Limit(limit))) if limit.resource == Resource::Steps)
    );
    assert_image(target.bytes.get_ref(), &before);
    assert_eq!((target.writes, journal_file.writes), (0, journal_writes));
    assert_image(journal_file.bytes.get_ref(), &journal_bytes);
    let mut forged = expected;
    forged[before.len() - 1] ^= 1;
    let (foreign_target, mut foreign_journal) = pending(&before, &forged);
    let checked = journal::inspect_v3(&mut foreign_journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery(checked, SCOPE, &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(foreign_target.writes, 0);
    assert_image(foreign_target.bytes.get_ref(), &before);
}

#[test]
fn shared_recovery_pool_accumulates_proof_and_native_work_without_refill() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let after = raw::numbers(&before, "Archive", &[Some(123), None]);
    let encoded = intent(&before, &[Some(123), None]);
    let (target, mut journal_file) = pending(&before, &after);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let mut measured = budget();
    drop(
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery_shared(checked, SCOPE, &mut measured, &|| false)
            .unwrap(),
    );
    let used = u64::MAX - measured.remaining(Resource::Steps);
    assert!(used > 1);
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let mut short = ResourceBudget::new(u64::MAX, u64::MAX, used - 1);
    assert!(matches!(
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap().qualify_recovery_shared(checked, SCOPE, &mut short, &|| false),
        Err(NativeRecoveryQualificationError::Inspect(InspectError::Limit(limit)))
            if limit.resource == Resource::Steps
    ));
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        NativePopulatedAddTableIntent::decode(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_recovery_shared(checked, SCOPE, &mut budget(), &|| true)
            .is_err()
    );
    assert_eq!(target.writes, 0);
}
