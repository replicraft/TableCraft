use super::{
    Observed, budget,
    goldens::{self, CASES},
    memory,
};
use crate::journal::{JournalError, RecoveryOutcome, recover_resizable};

#[test]
fn every_record_truncation_and_corruption_is_classified_before_target_writes() {
    for case in CASES {
        let complete = goldens::bytes(case.committed);
        let pending = goldens::bytes(case.pending);
        for length in 0..=complete.len() {
            let mut target = Observed::new(case.after);
            let mut journal = memory(&complete[..length]);
            let result = recover_resizable(&mut target, &mut journal, &mut budget(), &|| false);
            if length < pending.len() {
                assert_eq!(
                    result,
                    Err(JournalError::InvalidJournal),
                    "{} length {length}",
                    case.name
                );
                assert_eq!((target.writes, target.resizes), (0, 0));
                assert_eq!(target.storage.0.get_ref(), case.after);
            } else if length < complete.len() {
                assert_eq!(result, Ok(RecoveryOutcome::RolledBack));
                assert_eq!(target.storage.0.get_ref(), case.before);
            } else {
                assert_eq!(result, Ok(RecoveryOutcome::Committed));
                assert_eq!((target.writes, target.resizes), (0, 0));
                assert_eq!(target.storage.0.get_ref(), case.after);
            }
            assert_eq!(journal.0.get_ref(), &complete[..length]);
        }

        for offset in 0..complete.len() {
            let mut corrupted = complete.clone();
            corrupted[offset] ^= 0x80;
            let mut target = Observed::new(case.after);
            assert_eq!(
                recover_resizable(&mut target, &mut memory(&corrupted), &mut budget(), &|| {
                    false
                }),
                Err(JournalError::InvalidJournal),
                "{} offset {offset}",
                case.name
            );
            assert_eq!((target.writes, target.resizes), (0, 0));
        }
        let mut trailing = complete;
        trailing.push(0);
        let mut target = Observed::new(case.after);
        assert_eq!(
            recover_resizable(&mut target, &mut memory(&trailing), &mut budget(), &|| {
                false
            }),
            Err(JournalError::InvalidJournal)
        );
        assert_eq!((target.writes, target.resizes), (0, 0));
    }
}

#[test]
fn pending_lengths_and_real_tail_origins_restore_without_synthesized_bytes() {
    for case in CASES {
        let minimum = case.before.len().min(case.after.len());
        let maximum = case.before.len().max(case.after.len());
        for length in minimum..=maximum {
            let common = case.before.len().min(case.after.len());
            assert!(common <= 8, "bounded exhaustive literal masks");
            for mask in 0..(1_usize << common) {
                let current = (0..length)
                    .map(
                        |position| match (case.before.get(position), case.after.get(position)) {
                            (Some(before), Some(after)) => {
                                if mask & (1 << position) != 0 {
                                    *after
                                } else {
                                    *before
                                }
                            }
                            (Some(before), None) => *before,
                            (None, Some(after)) => *after,
                            (None, None) => unreachable!(),
                        },
                    )
                    .collect::<Vec<_>>();
                let mut target = memory(&current);
                let mut journal = memory(&goldens::bytes(case.pending));
                assert_eq!(
                    recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
                    RecoveryOutcome::RolledBack
                );
                assert_eq!(target.0.get_ref(), case.before);
            }
        }
    }

    let case = &CASES[0];
    for wrong in [
        b"rep\0".as_slice(),
        b"xx".as_slice(),
        b"replacement!".as_slice(),
    ] {
        let mut target = Observed::new(wrong);
        let mut journal = memory(&goldens::bytes(case.pending));
        assert_eq!(
            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
        assert_eq!((target.writes, target.resizes), (0, 0));
        assert_eq!(target.storage.0.get_ref(), wrong);
    }
}

#[test]
fn committed_foreign_or_partial_targets_refuse_without_rollforward() {
    let case = &CASES[0];
    for wrong in [case.before, b"replacemenX".as_slice(), b"repl".as_slice()] {
        let mut target = Observed::new(wrong);
        let mut journal = memory(&goldens::bytes(case.committed));
        assert_eq!(
            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
        assert_eq!((target.writes, target.resizes), (0, 0));
        assert_eq!(target.storage.0.get_ref(), wrong);
    }
}

#[test]
fn length_split_digest_and_oversized_headers_cannot_rebind_images() {
    let first = goldens::bytes(CASES[7].committed);
    let second = goldens::bytes(CASES[8].committed);
    let mut split = first;
    split[8..24].copy_from_slice(&second[8..24]);
    let mut target = Observed::new(b"c");
    assert_eq!(
        recover_resizable(&mut target, &mut memory(&split), &mut budget(), &|| false),
        Err(JournalError::InvalidJournal)
    );
    assert_eq!((target.writes, target.resizes), (0, 0));

    for offset in [8, 16] {
        let mut header = goldens::bytes(CASES[0].pending);
        header[offset..offset + 8].copy_from_slice(&(64_u64 * 1024 * 1024 + 1).to_le_bytes());
        let mut target = Observed::new(b"old");
        assert_eq!(
            recover_resizable(&mut target, &mut memory(&header), &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        );
        assert_eq!((target.writes, target.resizes), (0, 0));
    }
}
