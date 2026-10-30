use super::{CheckedV1, V1State, inspect_v1};
use crate::{
    IoError,
    journal::{DurableStorage, JournalError, RecoveryOutcome, recover},
};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

// Independent literal V1 source: original -> modified, length eight. No producer
// format constant, prepare call, digest routine or recovery constructs this wire.
const DIGEST: [u8; 32] = [
    0x37, 0x21, 0x25, 0x6c, 0x8b, 0x8e, 0x9a, 0xca, 0x2e, 0x07, 0x04, 0xa1, 0x1b, 0x4e, 0x30, 0x53,
    0x0c, 0xa7, 0x19, 0xea, 0xae, 0x4c, 0x6e, 0xf2, 0xc0, 0x49, 0x46, 0x01, 0xd0, 0xd2, 0x45, 0xac,
];
const BEFORE: &[u8; 8] = b"original";
const AFTER: &[u8; 8] = b"modified";

fn literal(marker_bytes: usize) -> Vec<u8> {
    assert!(marker_bytes <= 40);
    let mut result = b"TCIMG001\x08\0\0\0\0\0\0\0".to_vec();
    result.extend(DIGEST);
    result.extend(BEFORE);
    result.extend(AFTER);
    let mut marker = b"COMMIT01".to_vec();
    marker.extend(DIGEST);
    result.extend(&marker[..marker_bytes]);
    assert_eq!(result.len(), 64 + marker_bytes);
    result
}

#[derive(Default)]
struct Control {
    reads_denied: Cell<bool>,
    syncs_denied: Cell<bool>,
    writes_denied: Cell<bool>,
    reads: Cell<usize>,
    writes: Cell<usize>,
    syncs: Cell<usize>,
    events: RefCell<Vec<(&'static str, &'static str)>>,
}

struct Storage {
    bytes: Cursor<Vec<u8>>,
    name: &'static str,
    control: Rc<Control>,
}

impl Storage {
    fn new(bytes: Vec<u8>, name: &'static str, control: Rc<Control>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            name,
            control,
        }
    }
}

impl Read for Storage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert!(!self.control.reads_denied.get(), "journal was reloaded");
        self.control.reads.set(self.control.reads.get() + 1);
        self.control.events.borrow_mut().push((self.name, "read"));
        self.bytes.read(bytes)
    }
}

impl Write for Storage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert!(
            !self.control.writes_denied.get(),
            "inspection/journal wrote bytes"
        );
        self.control.writes.set(self.control.writes.get() + 1);
        self.control.events.borrow_mut().push((self.name, "write"));
        self.bytes.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush is not a storage barrier")
    }
}

impl Seek for Storage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.control.events.borrow_mut().push((self.name, "seek"));
        self.bytes.seek(position)
    }
}

impl DurableStorage for Storage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.control.events.borrow_mut().push((self.name, "length"));
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        assert!(
            !self.control.syncs_denied.get(),
            "inspection synchronized storage"
        );
        self.control.syncs.set(self.control.syncs.get() + 1);
        self.control.events.borrow_mut().push((self.name, "sync"));
        Ok(())
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn journal(bytes: Vec<u8>) -> (Storage, Rc<Control>) {
    let control = Rc::new(Control::default());
    control.writes_denied.set(true);
    (Storage::new(bytes, "journal", control.clone()), control)
}

fn assert_views(checked: &CheckedV1<'_, Storage>, marker_bytes: usize) {
    assert_eq!(checked.before_bytes(), BEFORE);
    assert_eq!(checked.after_bytes(), AFTER);
    assert_eq!(
        checked.state(),
        if marker_bytes == 40 {
            V1State::Committed
        } else {
            V1State::Pending
        }
    );
}

#[test]
fn qualify_checked_v1_literal_records_before_any_recovery_write() {
    for marker_bytes in 0..=40 {
        let original = literal(marker_bytes);
        let (mut bytes, control) = journal(original.clone());
        control.syncs_denied.set(true);
        let checked = inspect_v1(&mut bytes, &mut budget(), &|| false).unwrap();
        assert_views(&checked, marker_bytes);
        drop(checked);
        assert_eq!(control.writes.get(), 0);
        assert_eq!(control.syncs.get(), 0);
        assert_eq!(bytes.bytes.get_ref(), &original);
    }

    let complete = literal(40);
    for length in 0..64 {
        let (mut bytes, control) = journal(complete[..length].to_vec());
        control.syncs_denied.set(true);
        assert!(matches!(
            inspect_v1(&mut bytes, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
        assert_eq!(control.writes.get(), 0);
        assert_eq!(control.syncs.get(), 0);
    }
    for offset in 0..complete.len() {
        let mut corrupted = complete.clone();
        corrupted[offset] ^= 128;
        let (mut bytes, control) = journal(corrupted);
        control.syncs_denied.set(true);
        assert!(matches!(
            inspect_v1(&mut bytes, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
        assert_eq!(control.writes.get(), 0);
        assert_eq!(control.syncs.get(), 0);
    }
}

#[test]
fn checked_v1_recovery_retains_original_record_and_same_journal_without_reload() {
    for marker_bytes in 0..=40 {
        let original = literal(marker_bytes);
        let (mut bytes, journal_control) = journal(original.clone());
        let checked = inspect_v1(&mut bytes, &mut budget(), &|| false).unwrap();
        assert_views(&checked, marker_bytes);
        let reads = journal_control.reads.get();
        journal_control.reads_denied.set(true);
        let target_control = Rc::new(Control::default());
        let initial = if marker_bytes == 40 {
            AFTER.to_vec()
        } else {
            BEFORE
                .iter()
                .zip(AFTER)
                .enumerate()
                .map(|(i, (old, new))| if i % 2 == 0 { *old } else { *new })
                .collect()
        };
        let mut target = Storage::new(initial, "target", target_control.clone());
        let outcome = checked
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        let committed = marker_bytes == 40;
        assert_eq!(
            outcome,
            if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref().as_slice(),
            if committed { AFTER } else { BEFORE }
        );
        assert_eq!(target_control.writes.get(), usize::from(!committed));
        assert_eq!(target_control.syncs.get(), 1);
        assert_eq!(journal_control.reads.get(), reads);
        assert_eq!(journal_control.writes.get(), 0);
        assert_eq!(journal_control.syncs.get(), 1);
        assert_eq!(bytes.bytes.get_ref(), &original);
    }
}

#[test]
fn checked_and_existing_v1_recovery_have_identical_callbacks_and_cost() {
    for marker_bytes in [0, 1, 39, 40] {
        let mut observations = Vec::new();
        for use_checked in [false, true] {
            let control = Rc::new(Control::default());
            let mut target = Storage::new(AFTER.to_vec(), "target", control.clone());
            let mut bytes = Storage::new(literal(marker_bytes), "journal", control.clone());
            let mut allowance = budget();
            let outcome = if use_checked {
                inspect_v1(&mut bytes, &mut allowance, &|| false)
                    .unwrap()
                    .recover(&mut target, &mut allowance, &|| false)
            } else {
                recover(&mut target, &mut bytes, &mut allowance, &|| false)
            };
            let remaining = [Resource::Bytes, Resource::Rows, Resource::Steps]
                .map(|resource| allowance.remaining(resource));
            observations.push((
                outcome,
                remaining,
                control.events.borrow().clone(),
                target.bytes.into_inner(),
                bytes.bytes.into_inner(),
            ));
        }
        assert_eq!(observations[0], observations[1]);
    }
}

#[test]
fn checked_v1_inspection_exact_pools_and_final_cancellation_are_read_only() {
    let (mut bytes, control) = journal(literal(40));
    control.syncs_denied.set(true);
    let mut measured = budget();
    let polls = Cell::new(0);
    let checked = inspect_v1(&mut bytes, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_views(&checked, 40);
    drop(checked);
    let resources = [Resource::Bytes, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    for (position, resource) in resources.into_iter().enumerate() {
        for exact in [false, true] {
            let mut limits = [1_000_000, 1_000_000];
            limits[position] = used[position] - u64::from(!exact);
            let mut allowance = ResourceBudget::new(limits[0], 0, limits[1]);
            let (mut bytes, control) = journal(literal(40));
            control.syncs_denied.set(true);
            let result = inspect_v1(&mut bytes, &mut allowance, &|| false);
            assert_eq!(result.is_ok(), exact);
            if exact {
                assert_eq!(allowance.remaining(resource), 0);
            }
            drop(result);
            assert_eq!(control.writes.get(), 0);
            assert_eq!(control.syncs.get(), 0);
        }
    }

    let terminal = polls.get();
    assert!(terminal > 1);
    let (mut bytes, control) = journal(literal(40));
    control.syncs_denied.set(true);
    let calls = Cell::new(0);
    assert!(matches!(
        inspect_v1(&mut bytes, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() == terminal
        }),
        Err(JournalError::Io(IoError::Cancelled))
    ));
    assert_eq!(control.writes.get(), 0);
    assert_eq!(control.syncs.get(), 0);
}

#[test]
fn checked_v1_target_mismatch_does_not_write_or_sync_either_resource() {
    for marker_bytes in [0, 40] {
        let (mut bytes, journal_control) = journal(literal(marker_bytes));
        let checked = inspect_v1(&mut bytes, &mut budget(), &|| false).unwrap();
        let target_control = Rc::new(Control::default());
        target_control.writes_denied.set(true);
        target_control.syncs_denied.set(true);
        let mut target = Storage::new(b"unrelate".to_vec(), "target", target_control.clone());
        assert_eq!(
            checked.recover(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
        assert_eq!(target_control.writes.get(), 0);
        assert_eq!(target_control.syncs.get(), 0);
        assert_eq!(journal_control.writes.get(), 0);
        assert_eq!(journal_control.syncs.get(), 0);
    }
}
