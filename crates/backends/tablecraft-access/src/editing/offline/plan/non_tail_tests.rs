//! Journal-only hosts recognize TCFD5 but cannot acquire native schema authority.
use super::{Cause, MutationIntent};
use crate::editing::offline::{OfflineWriteSession, SessionState};
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_ace::FieldDropIntentError;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::filesystem::OfflineJournalPair;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn request(before: &[u8], definition: u32) -> Vec<u8> {
    let mut bytes = b"TCFD0005".to_vec();
    bytes.extend(87_u32.to_le_bytes());
    bytes.extend(5_u16.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    bytes.extend(hash.finalize());
    bytes.extend((definition as i32).to_le_bytes());
    bytes.extend(definition.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend(5_u16.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"Table1Score");
    bytes
}

#[test]
fn journal_only_non_tail_dispatch_recognizes_five_but_recovery_refuses_native_authority() {
    let source = fields::source(fields::Case::FirstNumber);
    let raw = request(&source.bytes, source.definition);
    assert!(
        matches!(MutationIntent::decode(&raw, &mut budget(), &|| false),
        Ok(MutationIntent::RetainedFieldDrop(intent)) if intent.is_retained_non_tail())
    );
    let mut mixed = raw.clone();
    mixed[7] = b'4';
    assert!(matches!(
        MutationIntent::decode(&mixed, &mut budget(), &|| false),
        Err(Cause::FieldDropIntent(FieldDropIntentError::InvalidRecord))
    ));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("journal-only.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &source.bytes).unwrap();
    let mut owner =
        OfflineWriteSession::open(&path, CatalogScope::new(1615), &mut budget(), &|| false)
            .unwrap();
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &bundle,
        raw,
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
                &source.bytes,
                &source.bytes,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    let failure = owner
        .recover_pending(&mut budget(), &|| false)
        .err()
        .expect("journal-only code has no native schema-write authority");
    assert!(matches!(
        failure.cause(),
        Cause::FieldDropIntent(FieldDropIntentError::UnsupportedSource)
    ));
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), source.bytes);
}
