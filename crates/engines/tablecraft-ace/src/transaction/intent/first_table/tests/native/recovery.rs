//! Native construction is requalified through the same checked V3 session.
use crate::{
    CreateColumn, FirstTableDefinition, NativeFirstTableIntent, NativeImage,
    NativeRecoveryQualificationError, qualify_native_first_table_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::{self, JournalError, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::{
    fixtures::access::canonical_wide_table_creation as raw, support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

fn definition(table: &str) -> FirstTableDefinition {
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
    FirstTableDefinition::capture_native(table, &columns, &mut budget(), &|| false).unwrap()
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

fn journal_literal(before: &[u8], after: &[u8], committed: bool) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCIMG003");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update((after.len() as u64).to_le_bytes());
    hash.update(before);
    hash.update(after);
    let digest = hash.finalize();
    let mut bytes = b"TCIMG003".to_vec();
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend((after.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&digest);
    bytes.extend(before);
    bytes.extend(after);
    if committed {
        bytes.extend(b"COMMIT03");
        bytes.extend_from_slice(&digest);
    }
    bytes
}

#[test]
fn native_wide_first_table_rebuilds_independent_images_for_pending_and_committed_recovery() {
    let source = raw::source();
    for committed in [false, true] {
        let plan = image(&source.before, 7081)
            .plan_first_table_native(definition("Table1"), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), source.after);
        let intent_bytes = plan.encode_intent(&mut budget(), &|| false).unwrap();
        assert_eq!(&intent_bytes[..8], b"TCNT0007");
        let mut target = MemoryStorage::new(source.before.clone());
        let mut journal_file = MemoryStorage::new(Vec::new());
        let prepared = plan
            .prepare_native(&mut target, &mut journal_file, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes = Cursor::new(source.after.clone());
        }
        assert_eq!(
            journal_file.bytes.get_ref(),
            &journal_literal(&source.before, &source.after, committed)
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let qualified =
            NativeFirstTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
                .unwrap()
                .qualify_native_recovery(checked, CatalogScope::new(7082), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(
            qualified.state(),
            if committed {
                ResizableState::Committed
            } else {
                ResizableState::Pending
            }
        );
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
            if committed {
                &source.after
            } else {
                &source.before
            }
        );
        let writes = target.writes;
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        NativeFirstTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
            .unwrap()
            .qualify_native_recovery(checked, CatalogScope::new(7083), &mut budget(), &|| false)
            .unwrap()
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn native_first_table_old_prepare_wrong_protocol_and_forged_after_have_no_target_effects() {
    let source = raw::source();
    let plan = image(&source.before, 7091)
        .plan_first_table_native(definition("Table1"), &mut budget(), &|| false)
        .unwrap();
    let intent_bytes = plan.encode_intent(&mut budget(), &|| false).unwrap();
    let mut target = MemoryStorage::new(source.before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::InvalidJournal)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal_file,
            &source.before,
            &source.after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_first_table_recovery(
            checked,
            NativeFirstTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(7092),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
    let mut forged = source.after.clone();
    *forged.last_mut().unwrap() ^= 1;
    let mut journal_file = MemoryStorage::new(journal_literal(&source.before, &forged, false));
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeFirstTableIntent::decode_native(&intent_bytes, &mut budget(), &|| false)
            .unwrap()
            .qualify_native_recovery(checked, CatalogScope::new(7093), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(journal_file.writes, 0);
    assert_eq!(target.writes, 0);
}
