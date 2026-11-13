//! Data-only TCQC dispatch is terminal but grants no unavailable native authority.
use super::{Cause, MutationIntent};
use crate::editing::offline::{OfflineWriteSession, SessionState};
use sha2::{Digest, Sha256};
use std::fs;
use tablecraft_ace::QueryCreationIntentError;
use tablecraft_core::{limits::ResourceBudget, values::LegacyDateTime};
use tablecraft_io::journal::filesystem::OfflineJournalPair;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::query_expression_edits;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn original(before: &[u8]) -> Vec<u8> {
    // Independent literal TCQC1 framing. Data alone is never an apply capability.
    let mut bytes = b"TCQC0001".to_vec();
    bytes.extend(93_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    bytes.extend((i32::MIN + 124).to_le_bytes());
    bytes.extend(43_i32.to_le_bytes());
    bytes.extend(
        LegacyDateTime::from_ole_days(45000.5)
            .unwrap()
            .to_bits()
            .to_le_bytes(),
    );
    bytes.extend(7_u16.to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"CreatedAnswer");
    bytes
}

#[test]
fn tcqc_data_and_version_dispatch_remain_closed_without_native_creation_authority() {
    let before = query_expression_edits::source();
    let mut raw = original(&before);
    assert!(matches!(
        MutationIntent::decode(&raw, &mut budget(), &|| false),
        Ok(MutationIntent::QueryCreation(_))
    ));
    raw[7] = b'2';
    assert!(matches!(
        MutationIntent::decode(&raw, &mut budget(), &|| false),
        Err(Cause::QueryCreationIntent(
            QueryCreationIntentError::UnsupportedVersion
        ))
    ));
    raw[7] = b'1';

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("journal-only.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, &before).unwrap();
    let mut owner =
        OfflineWriteSession::open(&path, CatalogScope::new(1932), &mut budget(), &|| false)
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
                &before,
                &before,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    let failure = match owner.recover_pending(&mut budget(), &|| false) {
        Err(failure) => failure,
        Ok(_) => panic!("journal-only QueryDef data cannot acquire native creation authority"),
    };
    assert!(matches!(
        failure.cause(),
        Cause::QueryCreationIntent(QueryCreationIntentError::UnsupportedSource)
    ));
    failure.into_session().release().unwrap();
    assert_eq!(fs::read(path).unwrap(), before);
}
