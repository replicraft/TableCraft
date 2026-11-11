//! Original TCNA10 recognition is independent of optional native write authority.
use super::{Cause, MutationIntent};
#[cfg(not(feature = "ace16-create"))]
use crate::editing::offline::{OfflineWriteSession, SessionState};
use sha2::{Digest, Sha256};
#[cfg(not(feature = "ace16-create"))]
use std::fs;
use tablecraft_ace::NativeIntentError;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_ace::{EditError, NativeRecoveryQualificationError, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_io::journal::filesystem::OfflineJournalPair;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{field_changes, fields},
    tcna2,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let populated = field_changes::keyless_first_field(&seed, "Keyless");
    (
        field_changes::drop_field(&populated, definition, root, "Id"),
        definition,
    )
}

fn literal(before: &[u8]) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    let mut wire = tcna2::wider_presence_wire(
        "Keyless",
        before.len(),
        hash.finalize().into(),
        vec![vec![None]],
    );
    wire[..8].copy_from_slice(b"TCNA0010");
    wire[12..14].copy_from_slice(&10_u16.to_le_bytes());
    wire
}

#[test]
fn original_keyless_presence_dispatch_selects_the_exact_retained_v3_family() {
    let (before, _) = source();
    let wire = literal(&before);
    assert!(matches!(
        MutationIntent::decode(&wire, &mut budget(), &|| false),
        Ok(MutationIntent::RetainedInsert(intent)) if intent.is_retained_keyless()
            && !intent.is_retained() && !intent.is_retained_opaque()
    ));
    let mut mismatched = wire;
    mismatched[12..14].copy_from_slice(&9_u16.to_le_bytes());
    assert!(matches!(
        MutationIntent::decode(&mismatched, &mut budget(), &|| false),
        Err(Cause::Intent(NativeIntentError::UnsupportedProfile))
    ));
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_keyless_v3_recovery_preserves_the_target_and_retained_journal() {
    let (before, definition) = source();
    let after = field_changes::inserting::keyless_score(&before, definition, None);
    let wire = literal(&before);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("keyless.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let mut owner =
        OfflineWriteSession::open(&path, CatalogScope::new(4630), &mut budget(), &|| false)
            .unwrap();
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &bundle,
        wire,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        drop(
            tablecraft_io::journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal,
                &before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    let journal_before = fs::read(bundle.join("journal")).unwrap();
    let intent_before = fs::read(bundle.join("intent")).unwrap();
    let failure = owner
        .recover_pending(&mut budget(), &|| false)
        .err()
        .expect("journal-only hosts cannot reconstruct native keyless insertion");
    assert!(matches!(
        failure.cause(),
        Cause::Qualification(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(fs::read(bundle.join("journal")).unwrap(), journal_before);
    assert_eq!(fs::read(bundle.join("intent")).unwrap(), intent_before);
}
