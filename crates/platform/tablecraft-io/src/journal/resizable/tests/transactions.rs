use super::{
    Observed, budget,
    goldens::{self, CASES},
    memory,
};
use crate::{
    IoError,
    journal::{
        JournalError, RecoveryOutcome, prepare, prepare_resizable_expected, recover,
        recover_resizable,
    },
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[test]
fn produced_records_and_repeated_outcomes_match_all_independent_goldens() {
    for case in CASES {
        for committed in [false, true] {
            let mut target = memory(case.before);
            let mut journal = memory(&[]);
            let prepared = prepare_resizable_expected(
                &mut target,
                &mut journal,
                case.before,
                case.after,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
            }
            assert_eq!(
                journal.0.get_ref(),
                &goldens::bytes(if committed {
                    case.committed
                } else {
                    case.pending
                }),
                "{}",
                case.name
            );
            if !committed {
                assert_eq!(target.0.get_ref(), case.before);
            }

            for _ in 0..2 {
                let result =
                    recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap();
                assert_eq!(
                    result,
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(
                    target.0.get_ref(),
                    if committed { case.after } else { case.before }
                );
            }
        }

        let mut target = memory(case.before);
        let mut journal = memory(&[]);
        assert_eq!(
            prepare_resizable_expected(
                &mut target,
                &mut journal,
                case.before,
                case.after,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rollback(&mut budget(), &|| false)
            .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.0.get_ref(), case.before);
    }
}

#[test]
fn expected_identity_and_journal_reuse_refuse_before_any_writes() {
    for expected in [b"bad".as_slice(), b"wrong-length".as_slice()] {
        let mut target = Observed::new(b"old");
        let mut journal = Observed::new(&[]);
        assert_eq!(
            prepare_resizable_expected(
                &mut target,
                &mut journal,
                expected,
                b"replacement",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(JournalError::TargetMismatch)
        );
        assert_eq!(
            (
                target.writes,
                target.resizes,
                journal.writes,
                journal.resizes
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(target.storage.0.get_ref(), b"old");
        assert!(journal.storage.0.get_ref().is_empty());
    }

    let mut target = Observed::new(b"old");
    let mut journal = Observed::new(b"retained");
    assert_eq!(
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            b"old",
            b"replacement",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(JournalError::JournalNotEmpty)
    );
    assert_eq!((target.writes, target.resizes, journal.writes), (0, 0, 0));
    assert_eq!(journal.storage.0.get_ref(), b"retained");
}

#[test]
fn fixed_v1_dispatch_and_old_v2_refusal_keep_their_wire_contracts() {
    for committed in [false, true] {
        let mut target = memory(b"original");
        let mut journal = memory(&[]);
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
        assert_eq!(&journal.0.get_ref()[..16], b"TCIMG001\x08\0\0\0\0\0\0\0");
        let retained = journal.0.get_ref().clone();
        assert_eq!(
            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(journal.0.get_ref(), &retained);
    }

    let case = &CASES[0];
    let mut target = Observed::new(case.after);
    let mut journal = memory(&goldens::bytes(case.pending));
    assert_eq!(
        recover(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::InvalidJournal)
    );
    assert_eq!((target.writes, target.resizes), (0, 0));
}

#[test]
fn exact_pools_and_final_cancellation_retain_recoverable_state() {
    let before = vec![0x51; 70_003];
    let after = vec![0x92; 131_077];
    let mut target = memory(&before);
    let mut journal = memory(&[]);
    let mut work = budget();
    let pools = [Resource::Bytes, Resource::Steps];
    let initial = pools.map(|pool| work.remaining(pool));
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    prepare_resizable_expected(
        &mut target,
        &mut journal,
        &before,
        &after,
        &mut work,
        &cancelled,
    )
    .unwrap()
    .commit(&mut work, &cancelled)
    .unwrap();
    let needed =
        std::array::from_fn::<_, 2, _>(|index| initial[index] - work.remaining(pools[index]));

    for index in 0..2 {
        for shortage in [false, true] {
            let mut limits = [32 * 1024 * 1024; 2];
            limits[index] = needed[index] - u64::from(shortage);
            let mut work = ResourceBudget::new(limits[0], 0, limits[1]);
            let mut target = memory(&before);
            let mut journal = memory(&[]);
            let result = prepare_resizable_expected(
                &mut target,
                &mut journal,
                &before,
                &after,
                &mut work,
                &|| false,
            )
            .map(|prepared| prepared.commit(&mut work, &|| false));
            if shortage {
                assert!(result.is_err() || result.unwrap().is_err());
            } else {
                result.unwrap().unwrap();
                assert_eq!(target.0.get_ref(), &after);
            }
            let outcome =
                recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap();
            assert_eq!(
                target.0.get_ref(),
                if outcome == RecoveryOutcome::Committed {
                    &after
                } else {
                    &before
                }
            );
        }
    }

    let stop = calls.get();
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() == stop
    };
    let mut target = memory(&before);
    let mut journal = memory(&[]);
    let prepared = prepare_resizable_expected(
        &mut target,
        &mut journal,
        &before,
        &after,
        &mut budget(),
        &cancel,
    )
    .unwrap();
    assert!(prepared.commit(&mut budget(), &cancel).is_err());
    assert_eq!(
        recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!(target.0.get_ref(), &after);

    let mut target = Observed::new(b"old");
    let mut journal = Observed::new(&[]);
    assert_eq!(
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            b"old",
            b"replacement",
            &mut budget(),
            &|| true
        )
        .err(),
        Some(JournalError::Io(IoError::Cancelled))
    );
    assert_eq!((target.writes, target.resizes, journal.writes), (0, 0, 0));
}
