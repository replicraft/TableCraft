//! Generated recovery states use independent wire bytes, never a producer API.
use super::{Observed, budget, memory};
use crate::journal::{JournalError, RecoveryOutcome, recover_resizable};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn record(before: &[u8], after: &[u8], marker_bytes: usize) -> Vec<u8> {
    let mut prefix = b"TCIMG002".to_vec();
    prefix.extend((before.len() as u64).to_le_bytes());
    prefix.extend((after.len() as u64).to_le_bytes());

    let mut hash = Sha256::new();
    hash.update(&prefix);
    hash.update(before);
    hash.update(after);
    let digest = hash.finalize();

    let mut bytes = prefix;
    bytes.extend_from_slice(&digest);
    bytes.extend_from_slice(before);
    bytes.extend_from_slice(after);

    let marker = [b"COMMIT02".as_slice(), digest.as_slice()].concat();
    bytes.extend_from_slice(&marker[..marker_bytes]);
    bytes
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x524553495a4532),
        ..Default::default()
    })]

    #[test]
    fn generated_pending_images_restore_only_real_before_bytes(
        before in prop::collection::vec(any::<u8>(), 0..80),
        after in prop::collection::vec(any::<u8>(), 0..80),
        choices in prop::collection::vec(any::<bool>(), 80),
        length_selector in any::<usize>(),
        marker_bytes in 0_usize..40,
    ) {
        let minimum = before.len().min(after.len());
        let maximum = before.len().max(after.len());
        let length = minimum + length_selector % (maximum - minimum + 1);
        let current = (0..length)
            .map(|position| {
                match (before.get(position), after.get(position)) {
                    (Some(old), Some(new)) => {
                        if choices[position] {
                            *new
                        } else {
                            *old
                        }
                    }
                    (Some(old), None) => *old,
                    (None, Some(new)) => *new,
                    (None, None) => unreachable!(),
                }
            })
            .collect::<Vec<_>>();
        let bytes = record(&before, &after, marker_bytes);
        let mut target = memory(&current);
        let mut journal = memory(&bytes);

        for _ in 0..2 {
            prop_assert_eq!(
                recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
                Ok(RecoveryOutcome::RolledBack)
            );
            prop_assert_eq!(target.0.get_ref(), &before);
            prop_assert_eq!(journal.0.get_ref(), &bytes);
        }
    }

    #[test]
    fn generated_committed_images_are_verified_without_target_rewrites(
        before in prop::collection::vec(any::<u8>(), 0..80),
        after in prop::collection::vec(any::<u8>(), 0..80),
    ) {
        let bytes = record(&before, &after, 40);
        let mut target = Observed::new(&after);
        let mut journal = memory(&bytes);
        for _ in 0..2 {
            prop_assert_eq!(
                recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
                Ok(RecoveryOutcome::Committed)
            );
            prop_assert_eq!((target.writes, target.resizes), (0, 0));
            prop_assert_eq!(target.storage.0.get_ref(), &after);
            prop_assert_eq!(journal.0.get_ref(), &bytes);
        }

        let mut foreign = after.clone();
        if let Some(first) = foreign.first_mut() {
            *first ^= 0xff;
        } else {
            foreign.push(0);
        }
        let mut target = Observed::new(&foreign);
        prop_assert_eq!(
            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
        prop_assert_eq!((target.writes, target.resizes), (0, 0));
        prop_assert_eq!(target.storage.0.get_ref(), &foreign);
        prop_assert_eq!(journal.0.get_ref(), &bytes);
    }

    #[test]
    fn generated_record_corruption_never_writes_either_resource(
        before in prop::collection::vec(any::<u8>(), 0..80),
        after in prop::collection::vec(any::<u8>(), 0..80),
        marker_bytes in 0_usize..=40,
        offset_selector in any::<usize>(),
        difference in 1_u8..=u8::MAX,
    ) {
        let mut bytes = record(&before, &after, marker_bytes);
        let offset = offset_selector % bytes.len();
        bytes[offset] ^= difference;
        let mut target = Observed::new(&after);
        let mut journal = Observed::new(&bytes);

        prop_assert_eq!(
            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        );
        prop_assert_eq!((target.writes, target.resizes), (0, 0));
        prop_assert_eq!((journal.writes, journal.resizes), (0, 0));
        prop_assert_eq!(target.storage.0.get_ref(), &after);
        prop_assert_eq!(journal.storage.0.get_ref(), &bytes);
    }
}
