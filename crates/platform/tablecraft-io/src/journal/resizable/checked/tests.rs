//! Reuse independent V2 wire goldens; no transaction writer supplies expectations.
use super::{V2State, inspect_v2};
use crate::{
    IoError,
    journal::{
        DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
        resizable::tests::goldens::{CASES, bytes},
    },
};
use std::{
    array,
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Default)]
struct Control {
    deny_reads: Cell<bool>,
    deny_writes: Cell<bool>,
    deny_sync: Cell<bool>,
    reads: Cell<usize>,
    writes: Cell<usize>,
    resizes: Cell<usize>,
    syncs: Cell<usize>,
}

struct Storage {
    bytes: Cursor<Vec<u8>>,
    control: Rc<Control>,
}

impl Storage {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            control: Rc::new(Control::default()),
        }
    }
}

impl Read for Storage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert!(
            !self.control.deny_reads.get(),
            "inspected journal was reloaded"
        );
        self.control.reads.set(self.control.reads.get() + 1);
        self.bytes.read(bytes)
    }
}

impl Write for Storage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert!(!self.control.deny_writes.get(), "unexpected storage write");
        self.control.writes.set(self.control.writes.get() + 1);
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush must not replace durability acknowledgment")
    }
}

impl Seek for Storage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl DurableStorage for Storage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.control.syncs.set(self.control.syncs.get() + 1);
        if self.control.deny_sync.get() {
            return Err(IoError::Io(io::ErrorKind::Other));
        }
        Ok(())
    }
}

impl ResizableDurableStorage for Storage {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        assert!(!self.control.deny_writes.get(), "unexpected storage resize");
        self.control.resizes.set(self.control.resizes.get() + 1);
        self.bytes
            .get_mut()
            .resize(usize::try_from(length).unwrap(), 0);
        Ok(())
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn v2_inspection_retains_all_literal_views_and_never_effects_or_reloads_journal() {
    for case in CASES {
        let pending = bytes(case.pending);
        let committed = bytes(case.committed);
        for marker in [0, 1, 7, 39, 40] {
            let wire = committed[..pending.len() + marker].to_vec();
            let mut journal = Storage::new(wire.clone());
            let control = journal.control.clone();
            control.deny_writes.set(true);
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.before_bytes(), case.before);
            assert_eq!(checked.after_bytes(), case.after);
            assert_eq!(
                checked.state(),
                if marker == 40 {
                    V2State::Committed
                } else {
                    V2State::Pending
                }
            );
            assert_eq!(control.writes.get(), 0);
            assert_eq!(control.syncs.get(), 0);
            control.deny_reads.set(true);
            let mut target = Storage::new(case.after.to_vec());
            let outcome = checked
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            let (expected, image) = if marker == 40 {
                (RecoveryOutcome::Committed, case.after)
            } else {
                (RecoveryOutcome::RolledBack, case.before)
            };
            assert_eq!(outcome, expected);
            assert_eq!(target.bytes.get_ref(), image);
            assert_eq!(journal.bytes.get_ref(), &wire);
            assert_eq!(control.writes.get(), 0);
            assert_eq!(control.syncs.get(), 1);
        }
    }
}

#[test]
fn v2_confirmation_requires_exact_settled_bytes_without_any_write_or_resize() {
    for case in CASES {
        for committed in [false, true] {
            let wire = bytes(if committed {
                case.committed
            } else {
                case.pending
            });
            let expected = if committed { case.after } else { case.before };
            let mut journal = Storage::new(wire.clone());
            let journal_control = journal.control.clone();
            journal_control.deny_writes.set(true);
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            journal_control.deny_reads.set(true);
            let mut target = Storage::new(expected.to_vec());
            target.control.deny_writes.set(true);
            assert_eq!(
                checked
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(target.control.writes.get(), 0);
            assert_eq!(target.control.resizes.get(), 0);
            assert_eq!(target.control.syncs.get(), 1);
            assert_eq!(journal_control.syncs.get(), 1);
            assert_eq!(journal.bytes.get_ref(), &wire);
        }
    }
}

#[test]
fn v2_mixed_length_recovery_preserves_origins_but_confirmation_refuses_partial_image() {
    let case = &CASES[0];
    for length in case.before.len()..=case.after.len() {
        let mut mixed = case.after[..length].to_vec();
        mixed[..case.before.len()].copy_from_slice(case.before);
        let mut journal = Storage::new(bytes(case.pending));
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut target = Storage::new(mixed.clone());
        assert_eq!(
            checked
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.bytes.get_ref(), case.before);
        if mixed != case.before {
            let mut journal = Storage::new(bytes(case.pending));
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            let mut target = Storage::new(mixed);
            target.control.deny_writes.set(true);
            assert_eq!(
                checked.confirm_completed(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            );
            assert_eq!(target.control.syncs.get(), 0);
            assert_eq!(journal.control.syncs.get(), 0);
        }
    }
    for bad in [b"unknown".to_vec(), vec![0; case.after.len() + 1]] {
        let mut journal = Storage::new(bytes(case.pending));
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut target = Storage::new(bad);
        target.control.deny_writes.set(true);
        assert_eq!(
            checked.recover(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
    }
}

#[test]
fn v2_inspection_budget_cancel_and_corruption_fail_without_storage_effects() {
    let wire = bytes(CASES[0].committed);
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut measured = budget();
    let initial = pools.map(|pool| measured.remaining(pool));
    let mut journal = Storage::new(wire.clone());
    drop(inspect_v2(&mut journal, &mut measured, &|| false).unwrap());
    let costs: [u64; 3] = array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
    assert_eq!(costs[1], 0);
    let mut journal = Storage::new(wire.clone());
    drop(
        inspect_v2(
            &mut journal,
            &mut ResourceBudget::new(costs[0], 0, costs[2]),
            &|| false,
        )
        .unwrap(),
    );
    for (index, pool) in [(0, Resource::Bytes), (2, Resource::Steps)] {
        let mut below = costs;
        below[index] -= 1;
        let mut journal = Storage::new(wire.clone());
        assert!(matches!(
            inspect_v2(
                &mut journal,
                &mut ResourceBudget::new(below[0], 0, below[2]),
                &|| false,
            ),
            Err(JournalError::Limit(error)) if error.resource == pool
        ));
        assert_eq!(
            (journal.control.writes.get(), journal.control.syncs.get()),
            (0, 0)
        );
    }
    let polls = Cell::new(0_usize);
    let mut journal = Storage::new(wire.clone());
    drop(
        inspect_v2(&mut journal, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    for stop in [1, polls.get().div_ceil(2), polls.get()] {
        let seen = Cell::new(0_usize);
        let mut journal = Storage::new(wire.clone());
        assert!(matches!(
            inspect_v2(&mut journal, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            }),
            Err(JournalError::Io(IoError::Cancelled))
        ));
        assert_eq!(
            (journal.control.writes.get(), journal.control.syncs.get()),
            (0, 0)
        );
    }
    for at in [7, 8, 16, 24, 56, wire.len() - 1] {
        let mut bad = wire.clone();
        bad[at] ^= 1;
        let mut journal = Storage::new(bad);
        assert!(matches!(
            inspect_v2(&mut journal, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
        assert_eq!(
            (journal.control.writes.get(), journal.control.syncs.get()),
            (0, 0)
        );
    }
}

#[test]
fn v2_confirmation_barrier_failure_never_writes_or_reports_success() {
    for journal_failure in [false, true] {
        let case = &CASES[0];
        let mut journal = Storage::new(bytes(case.committed));
        let journal_control = journal.control.clone();
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut target = Storage::new(case.after.to_vec());
        target.control.deny_writes.set(true);
        target.control.deny_sync.set(!journal_failure);
        journal_control.deny_sync.set(journal_failure);
        assert!(
            checked
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(
            (target.control.writes.get(), target.control.resizes.get()),
            (0, 0)
        );
        assert_eq!(journal.control.writes.get(), 0);
        assert_eq!(journal.control.syncs.get(), usize::from(journal_failure));
    }
}

#[test]
fn v2_confirmation_cancellation_keeps_target_and_journal_bytes_unchanged() {
    let case = &CASES[0];
    let wire = bytes(case.committed);
    let polls = Cell::new(0_usize);
    let mut journal = Storage::new(wire.clone());
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    let mut target = Storage::new(case.after.to_vec());
    checked
        .confirm_completed(&mut target, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    for stop in [1, polls.get().div_ceil(2), polls.get()] {
        let mut journal = Storage::new(wire.clone());
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut target = Storage::new(case.after.to_vec());
        target.control.deny_writes.set(true);
        let seen = Cell::new(0_usize);
        assert_eq!(
            checked.confirm_completed(&mut target, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            }),
            Err(JournalError::Io(IoError::Cancelled))
        );
        assert_eq!(target.bytes.get_ref(), case.after);
        assert_eq!(target.control.resizes.get(), 0);
        assert_eq!(journal.bytes.get_ref(), &wire);
        assert_eq!(journal.control.writes.get(), 0);
    }
}
