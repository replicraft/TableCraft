use super::*;
use crate::journal::{
    RecoveryOutcome, inspect_v1, inspect_v2, prepare, prepare_resizable_expected, recover,
    recover_resizable, resizable::tests::goldens, tests::MemoryStorage,
};
use std::{
    cell::{Cell, RefCell},
    io::Cursor,
    rc::Rc,
};
use tablecraft_core::limits::Resource;

const PREFIX: &[u8] = b"host-owned binding and intent";

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

fn memory(bytes: &[u8]) -> MemoryStorage {
    MemoryStorage(Cursor::new(bytes.to_vec()))
}

#[test]
fn region_runs_actual_v1_commit_and_pending_recovery_without_touching_prefix() {
    for (deferred, committed) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut backing = memory(PREFIX);
        let mut target = memory(b"original");
        {
            let mut region = if deferred {
                BoundedJournalRegion::deferred(&mut backing, PREFIX.len() as u64, 1024).unwrap()
            } else {
                BoundedJournalRegion::new(
                    &mut backing,
                    PREFIX.len() as u64,
                    1024,
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
            };
            let prepared = prepare(
                &mut target,
                &mut region,
                b"modified",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.0.get_mut()[..4].copy_from_slice(b"modi");
            }
            inspect_v1(&mut region, &mut budget(), &|| false).unwrap();
            let expected = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            assert_eq!(
                recover(&mut target, &mut region, &mut budget(), &|| false).unwrap(),
                expected
            );
        }
        assert_eq!(&backing.0.get_ref()[..PREFIX.len()], PREFIX);
        assert_eq!(
            target.0.get_ref(),
            if committed { b"modified" } else { b"original" }
        );
    }
}

#[test]
fn region_runs_actual_v2_journal_without_claiming_resizable_storage() {
    let case = &goldens::CASES[0];
    let mut backing = memory(PREFIX);
    let mut target = memory(case.before);
    {
        let mut region = BoundedJournalRegion::new(
            &mut backing,
            PREFIX.len() as u64,
            1024,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        prepare_resizable_expected(
            &mut target,
            &mut region,
            case.before,
            case.after,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
        inspect_v2(&mut region, &mut budget(), &|| false).unwrap();
        assert_eq!(
            recover_resizable(&mut target, &mut region, &mut budget(), &|| false).unwrap(),
            RecoveryOutcome::Committed
        );
    }
    assert_eq!(&backing.0.get_ref()[..PREFIX.len()], PREFIX);
    assert_eq!(
        &backing.0.get_ref()[PREFIX.len()..],
        goldens::bytes(case.committed)
    );
    assert_eq!(target.0.get_ref(), case.after);
}

#[derive(Clone, Copy, Default)]
enum Fault {
    #[default]
    None,
    ReadInterrupted,
    WriteInterrupted,
    ReadCount,
    ReadPartial,
    WriteCount,
    WritePartial,
    SeekInterrupted,
    SeekWrong,
    LenMoves,
    LenOutside,
    Sync,
    SyncMoves,
    FlushMoves,
    Flush,
}

struct Observed {
    memory: MemoryStorage,
    fault: Rc<Cell<Fault>>,
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Observed {
    fn new(bytes: &[u8]) -> Self {
        Self {
            memory: memory(bytes),
            fault: Rc::default(),
            events: Rc::default(),
        }
    }

    fn take_fault(&self, applies: impl FnOnce(Fault) -> bool) -> Fault {
        let fault = self.fault.get();
        if applies(fault) {
            self.fault.take()
        } else {
            Fault::None
        }
    }
}

impl Read for Observed {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.events.borrow_mut().push("read");
        match self.take_fault(|fault| {
            matches!(
                fault,
                Fault::ReadInterrupted | Fault::ReadCount | Fault::ReadPartial
            )
        }) {
            Fault::ReadInterrupted => Err(ErrorKind::Interrupted.into()),
            Fault::ReadCount => Ok(output.len() + 1),
            Fault::ReadPartial => {
                self.memory.read_exact(&mut output[..1])?;
                Err(ErrorKind::Other.into())
            }
            _ => self.memory.read(output),
        }
    }
}

impl Write for Observed {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.events.borrow_mut().push("write");
        match self.take_fault(|fault| {
            matches!(
                fault,
                Fault::WriteInterrupted | Fault::WriteCount | Fault::WritePartial
            )
        }) {
            Fault::WriteInterrupted => Err(ErrorKind::Interrupted.into()),
            Fault::WriteCount => Ok(input.len() + 1),
            Fault::WritePartial => {
                self.memory.write_all(&input[..1])?;
                Err(ErrorKind::Other.into())
            }
            _ => self.memory.write(input),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.events.borrow_mut().push("flush");
        match self.take_fault(|fault| matches!(fault, Fault::FlushMoves | Fault::Flush)) {
            Fault::FlushMoves => {
                self.memory.seek(SeekFrom::Start(0))?;
            }
            Fault::Flush => return Err(ErrorKind::Other.into()),
            _ => {}
        }
        self.memory.flush()
    }
}

impl Seek for Observed {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.events.borrow_mut().push("seek");
        match self.take_fault(|fault| matches!(fault, Fault::SeekInterrupted | Fault::SeekWrong)) {
            Fault::SeekInterrupted => {
                self.memory.seek(SeekFrom::Start(0))?;
                Err(ErrorKind::Interrupted.into())
            }
            Fault::SeekWrong => {
                self.memory.seek(SeekFrom::Start(0))?;
                Ok(0)
            }
            _ => self.memory.seek(position),
        }
    }
}

impl DurableStorage for Observed {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.events.borrow_mut().push("length");
        match self.take_fault(|fault| matches!(fault, Fault::LenMoves | Fault::LenOutside)) {
            Fault::LenMoves => {
                self.memory.seek(SeekFrom::Start(0))?;
            }
            Fault::LenOutside => return Ok(u64::MAX),
            _ => {}
        }
        self.memory.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.events.borrow_mut().push("sync");
        match self.take_fault(|fault| matches!(fault, Fault::Sync | Fault::SyncMoves)) {
            Fault::SyncMoves => {
                self.memory.seek(SeekFrom::Start(0))?;
            }
            Fault::Sync => return Err(IoError::Io(ErrorKind::Other)),
            _ => {}
        }
        self.memory.sync_storage()
    }
}

#[test]
fn region_clamps_reads_rejects_holes_caps_and_seek_overflow_without_hidden_callbacks() {
    let mut backing = Observed::new(b"prefixbody");
    let events = backing.events.clone();
    {
        let mut region =
            BoundedJournalRegion::new(&mut backing, 6, 8, &mut budget(), &|| false).unwrap();
        events.borrow_mut().clear();
        let mut output = [0; 20];
        assert_eq!(region.read(&mut output).unwrap(), 4);
        assert_eq!(&output[..4], b"body");
        assert_eq!(*events.borrow(), ["read"]);
        assert_eq!(region.read(&mut output).unwrap(), 0);
        region.seek(SeekFrom::Start(7)).unwrap();
        let calls = events.borrow().len();
        assert!(region.write(b"x").is_err());
        assert!(region.seek(SeekFrom::Start(9)).is_err());
        assert!(region.seek(SeekFrom::Current(i64::MAX)).is_err());
        assert!(region.seek(SeekFrom::End(i64::MIN)).is_err());
        assert_eq!(events.borrow().len(), calls);
        region.seek(SeekFrom::End(0)).unwrap();
        assert!(region.write(b"12345").is_err());
        assert_eq!(region.write(b"1234").unwrap(), 4);
        region.flush().unwrap();
        region.sync_storage().unwrap();
    }
    assert_eq!(backing.memory.0.get_ref(), b"prefixbody1234");
    assert!(events.borrow().ends_with(&["write", "flush", "sync"]));
}

#[test]
fn region_requires_length_refresh_and_absolute_reseek_after_uncertain_transfers() {
    for fault in [
        Fault::ReadCount,
        Fault::ReadPartial,
        Fault::WriteCount,
        Fault::WritePartial,
    ] {
        let mut backing = Observed::new(b"prefixbody");
        let control = backing.fault.clone();
        let events = backing.events.clone();
        {
            let mut region =
                BoundedJournalRegion::new(&mut backing, 6, 8, &mut budget(), &|| false).unwrap();
            control.set(fault);
            if matches!(fault, Fault::ReadCount | Fault::ReadPartial) {
                assert!(region.read(&mut [0; 2]).is_err());
            } else {
                assert!(region.write(b"xy").is_err());
            }
            let calls = events.borrow().len();
            assert!(region.read(&mut [0]).is_err());
            assert!(region.write(b"z").is_err());
            assert!(region.stream_position().is_err());
            assert_eq!(events.borrow().len(), calls);
            region.seek(SeekFrom::Start(0)).unwrap();
            assert!(region.read(&mut [0]).is_err());
            control.set(Fault::LenMoves);
            assert_eq!(region.storage_len().unwrap(), 4);
            assert!(region.read(&mut [0]).is_err());
            region.seek(SeekFrom::Start(0)).unwrap();
            region.read_exact(&mut [0; 4]).unwrap();
        }
        assert_eq!(&backing.memory.0.get_ref()[..6], b"prefix");
    }
}

#[test]
fn region_preserves_bounded_interrupted_retry_and_real_sync_failures() {
    let mut backing = Observed::new(b"prefixbody");
    let control = backing.fault.clone();
    let mut region =
        BoundedJournalRegion::new(&mut backing, 6, 8, &mut budget(), &|| false).unwrap();
    control.set(Fault::SeekWrong);
    assert!(region.seek(SeekFrom::Start(0)).is_err());
    assert!(region.read(&mut [0]).is_err());
    control.set(Fault::SeekInterrupted);
    seek_absolute(&mut region, 0, &mut budget(), &|| false).unwrap();
    control.set(Fault::ReadInterrupted);
    crate::read_exact(&mut region, &mut [0; 4], &mut budget(), &|| false).unwrap();
    control.set(Fault::WriteInterrupted);
    crate::write_exact_at(&mut region, 0, b"next", &mut budget(), &|| false).unwrap();
    // Also establish that a direct interrupted write preserves logical position.
    region.seek(SeekFrom::Start(0)).unwrap();
    control.set(Fault::WriteInterrupted);
    assert_eq!(
        region.write(b"next").unwrap_err().kind(),
        ErrorKind::Interrupted
    );
    assert_eq!(region.write(b"next").unwrap(), 4);
    control.set(Fault::Sync);
    assert_eq!(region.sync_storage(), Err(IoError::Io(ErrorKind::Other)));
    control.set(Fault::LenOutside);
    assert!(region.storage_len().is_err());
    region.seek(SeekFrom::Start(0)).unwrap();
    assert!(region.write(b"x").is_err());
}

#[test]
fn moving_sync_and_flush_require_absolute_reseek_and_failed_barriers_require_length_refresh() {
    for fault in [Fault::SyncMoves, Fault::FlushMoves, Fault::Flush] {
        let mut backing = Observed::new(b"prefixbody");
        let control = backing.fault.clone();
        {
            let mut region =
                BoundedJournalRegion::new(&mut backing, 6, 8, &mut budget(), &|| false).unwrap();
            control.set(fault);
            let failed = matches!(fault, Fault::Flush);
            if matches!(fault, Fault::SyncMoves) {
                region.sync_storage().unwrap();
            } else {
                assert_eq!(region.flush().is_err(), failed);
            }
            assert!(region.read(&mut [0]).is_err());
            if failed {
                region.seek(SeekFrom::Start(0)).unwrap();
                assert!(region.write(b"x").is_err());
                region.storage_len().unwrap();
            }
            region.seek(SeekFrom::Start(0)).unwrap();
            let mut output = [0; 4];
            region.read_exact(&mut output).unwrap();
            assert_eq!(&output, b"body");
        }
        assert_eq!(backing.memory.0.get_ref(), b"prefixbody");
    }
}

#[test]
fn region_constructor_checks_geometry_resources_and_cancellation_before_callbacks() {
    for (prefix, cap, bytes) in [
        (5, 4, b"four".as_slice()),
        (1, 2, b"four".as_slice()),
        (u64::MAX, 1, b"".as_slice()),
    ] {
        let mut backing = Observed::new(bytes);
        assert!(
            BoundedJournalRegion::new(&mut backing, prefix, cap, &mut budget(), &|| false).is_err()
        );
        assert!(!backing.events.borrow().contains(&"seek"));
    }
    for cancelled in [false, true] {
        let mut backing = Observed::new(PREFIX);
        let mut work = ResourceBudget::new(0, 0, 0);
        let error =
            BoundedJournalRegion::new(&mut backing, PREFIX.len() as u64, 8, &mut work, &|| {
                cancelled
            })
            .err()
            .unwrap();
        assert!(matches!(error, IoError::Cancelled | IoError::Limit(_)));
        assert!(backing.events.borrow().is_empty());
    }
    let mut backing = Observed::new(PREFIX);
    let mut measured = budget();
    {
        let _region =
            BoundedJournalRegion::new(&mut backing, PREFIX.len() as u64, 8, &mut measured, &|| {
                false
            })
            .unwrap();
    }
    let paid = 1_000_000 - measured.remaining(Resource::Steps);
    let mut backing = Observed::new(PREFIX);
    let mut exact = ResourceBudget::new(0, 0, paid);
    {
        let _region =
            BoundedJournalRegion::new(&mut backing, PREFIX.len() as u64, 8, &mut exact, &|| false)
                .unwrap();
    }
    assert_eq!(exact.remaining(Resource::Steps), 0);
    let mut backing = Observed::new(PREFIX);
    let mut short = ResourceBudget::new(0, 0, paid - 1);
    assert!(matches!(
        BoundedJournalRegion::new(&mut backing, PREFIX.len() as u64, 8, &mut short, &|| false)
            .err(),
        Some(IoError::Limit(_))
    ));
}

#[test]
fn setup_checks_cancellation_after_length_and_seek_callbacks() {
    for stop in 1..=4 {
        let mut backing = Observed::new(PREFIX);
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert_eq!(
            BoundedJournalRegion::new(
                &mut backing,
                PREFIX.len() as u64,
                8,
                &mut budget(),
                &cancelled
            )
            .err(),
            Some(IoError::Cancelled)
        );
        assert_eq!(backing.memory.0.get_ref(), PREFIX);
    }
}
