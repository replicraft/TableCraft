use super::{MemoryStorage, budget};
use crate::{
    IoError,
    journal::{JournalError, RecoveryOutcome, prepare, recover},
};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

fn journal(committed: bool) -> Vec<u8> {
    let mut target = MemoryStorage(Cursor::new(b"original".to_vec()));
    let mut journal = MemoryStorage(Cursor::new(Vec::new()));
    let prepared = prepare(
        &mut target,
        &mut journal,
        b"modified",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if committed {
        prepared.commit(&mut budget(), &|| false).unwrap();
    } else {
        drop(prepared);
    }
    journal.0.into_inner()
}

#[test]
fn every_truncation_and_body_mutation_fails_closed_or_recovers_the_expected_state() {
    let complete = journal(true);
    // Independent wire facts: 8-byte magic, 8-byte LE size, 32-byte SHA-256,
    // two eight-byte images, then an eight-byte marker plus the same digest.
    assert_eq!(&complete[..16], b"TCIMG001\x08\0\0\0\0\0\0\0");
    assert_eq!(&complete[48..64], b"originalmodified");
    assert_eq!(&complete[64..72], b"COMMIT01");
    let digest: String = complete[16..48]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        digest,
        "3721256c8b8e9aca2e0704a11b4e30530ca719eaae4c6ef2c0494601d0d245ac"
    );
    assert_eq!(&complete[72..104], &complete[16..48]);

    for length in 0..=complete.len() {
        let mut target = MemoryStorage(Cursor::new(b"modified".to_vec()));
        let mut bytes = MemoryStorage(Cursor::new(complete[..length].to_vec()));
        let result = recover(&mut target, &mut bytes, &mut budget(), &|| false);
        if length < 64 {
            assert_eq!(result, Err(JournalError::InvalidJournal));
            assert_eq!(target.0.get_ref(), b"modified");
        } else if length < complete.len() {
            assert_eq!(result, Ok(RecoveryOutcome::RolledBack));
            assert_eq!(target.0.get_ref(), b"original");
        } else {
            assert_eq!(result, Ok(RecoveryOutcome::Committed));
            assert_eq!(target.0.get_ref(), b"modified");
        }
    }

    for offset in 0..complete.len() {
        let mut corrupted = complete.clone();
        corrupted[offset] ^= 0x80;
        let mut target = MemoryStorage(Cursor::new(b"modified".to_vec()));
        assert_eq!(
            recover(
                &mut target,
                &mut MemoryStorage(Cursor::new(corrupted)),
                &mut budget(),
                &|| false
            ),
            Err(JournalError::InvalidJournal)
        );
        assert_eq!(target.0.get_ref(), b"modified");
    }
}

#[test]
fn target_mismatch_and_existing_journals_never_get_overwritten() {
    let mut target = MemoryStorage(Cursor::new(b"unrelate".to_vec()));
    let mut bytes = MemoryStorage(Cursor::new(journal(false)));
    let original = bytes.0.get_ref().clone();

    assert_eq!(
        recover(&mut target, &mut bytes, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    );
    assert_eq!(target.0.get_ref(), b"unrelate");
    assert_eq!(
        prepare(
            &mut target,
            &mut bytes,
            b"different",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(JournalError::JournalNotEmpty)
    );
    assert_eq!(bytes.0.get_ref(), &original);

    let mut empty = MemoryStorage(Cursor::new(Vec::new()));
    assert_eq!(
        prepare(&mut target, &mut empty, b"short", &mut budget(), &|| false).err(),
        Some(JournalError::ImageSize)
    );
    assert!(empty.0.get_ref().is_empty());
}

#[test]
fn cancellation_and_limits_never_discard_the_prepared_journal() {
    for threshold in 1..50 {
        let mut target = MemoryStorage(Cursor::new(b"modified".to_vec()));
        let original = journal(false);
        let mut bytes = MemoryStorage(Cursor::new(original.clone()));
        let checks = Cell::new(0);
        let _ = recover(&mut target, &mut bytes, &mut budget(), &|| {
            checks.set(checks.get() + 1);
            checks.get() >= threshold
        });

        assert_eq!(bytes.0.get_ref(), &original);
        assert_eq!(
            recover(&mut target, &mut bytes, &mut budget(), &|| false).unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.0.get_ref(), b"original");
    }

    let mut target = MemoryStorage(Cursor::new(b"original".to_vec()));
    let mut empty = MemoryStorage(Cursor::new(Vec::new()));
    assert_eq!(
        prepare(&mut target, &mut empty, b"modified", &mut budget(), &|| {
            true
        })
        .err(),
        Some(JournalError::Io(IoError::Cancelled))
    );
    assert!(
        prepare(
            &mut target,
            &mut empty,
            b"modified",
            &mut ResourceBudget::new(0, 0, 100),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.0.get_ref(), b"original");
    assert!(empty.0.get_ref().is_empty());
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x4a4f55524e414c), ..Default::default()
    })]
    #[test]
    fn arbitrary_partial_target_writes_restore_the_original(mask in any::<u8>()) {
        let mixed: Vec<_> = b"original".iter().zip(b"modified").enumerate()
            .map(|(i, (before, after))| if mask & (1 << i) != 0 { *after } else { *before }).collect();
        let mut target = MemoryStorage(Cursor::new(mixed));
        let mut bytes = MemoryStorage(Cursor::new(journal(false)));
        prop_assert_eq!(recover(&mut target, &mut bytes, &mut budget(), &|| false).unwrap(), RecoveryOutcome::RolledBack);
        prop_assert_eq!(target.0.get_ref(), b"original");
    }

    #[test]
    fn arbitrary_journals_are_bounded_and_leave_the_target_unchanged_on_invalid_input(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut target = MemoryStorage(Cursor::new(b"original".to_vec()));
        let mut bytes = MemoryStorage(Cursor::new(bytes));
        let _ = recover(&mut target, &mut bytes, &mut ResourceBudget::new(4096, 0, 4096), &|| false);
        prop_assert_eq!(target.0.get_ref(), b"original");
    }
}
