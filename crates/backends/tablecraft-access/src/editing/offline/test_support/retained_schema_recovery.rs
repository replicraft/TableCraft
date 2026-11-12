//! Shared filesystem recovery driver for actual retained schema actions.
use crate::editing::offline::{
    CompletionKind, OfflineWriteSession, SchemaChange, SessionState, VerifiedImage,
    test_support::saved_field::retired_namespace,
};
use std::fs;
use std::path::Path;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, CatalogScope::new(17313), &mut budget(), &|| false).unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

pub(in crate::editing::offline) fn cold_case(
    before: &[u8],
    expected: &[u8],
    encoded: Vec<u8>,
    committed: bool,
    retired: bool,
    schema_change: SchemaChange,
    prepare: impl FnOnce(&mut OfflineJournalPair, bool, bool),
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("declared.accdb");
    let bundle = directory.path().join("journal");
    fs::write(&path, before).unwrap();
    let mut owner = open(&path);
    let slot = owner.slot_mut().unwrap();
    let mut pair = OfflineJournalPair::create(
        slot.target.take().unwrap(),
        &bundle,
        encoded,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    prepare(&mut pair, committed, retired);
    if retired {
        pair.retire(if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        })
        .unwrap();
    }
    slot.pair = Some(pair);
    slot.state = SessionState::RetainedJournal;
    owner.release().unwrap();
    let (owner, receipt) = if retired {
        open(&path)
            .confirm_retired(&retired_namespace(directory.path()), &mut budget(), &|| {
                false
            })
            .unwrap()
    } else {
        open(&path)
            .recover_update(&bundle, &mut budget(), &|| false)
            .unwrap()
    };
    assert_eq!(receipt.schema_change(), committed.then_some(schema_change));
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (None, None)
    );
    if retired {
        assert_eq!(receipt.kind(), CompletionKind::Confirmed);
        assert_eq!(
            receipt.verified_image(),
            Some(if committed {
                VerifiedImage::Replacement
            } else {
                VerifiedImage::Original
            })
        );
    } else {
        assert_eq!(receipt.kind(), CompletionKind::Recovered);
        assert_eq!(
            receipt.outcome(),
            Some(if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            })
        );
    }
    owner.release().unwrap();
    same(
        &fs::read(&path).unwrap(),
        if committed { expected } else { before },
    );
}
