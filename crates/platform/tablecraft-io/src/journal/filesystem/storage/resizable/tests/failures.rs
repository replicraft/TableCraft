//! Actual retained pairs survive resizable protocol and namespace failures.
use super::{budget, pair};
use crate::{
    IoError, SharedFile,
    journal::{
        DurableStorage, RecoveryOutcome, ResizableDurableStorage, filesystem::PairError,
        inspect_v2, prepare_resizable_expected,
    },
};
use std::{
    cell::Cell,
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

#[derive(Clone, Copy)]
enum ResizeFault {
    Before,
    Partial,
    After,
}

#[derive(Default)]
struct Control {
    resize: Cell<Option<ResizeFault>>,
    sync: Cell<bool>,
}

struct Fault<S> {
    storage: S,
    control: Rc<Control>,
}

impl<S> Fault<S> {
    fn new(storage: S, control: Rc<Control>) -> Self {
        Self { storage, control }
    }
}

impl<S: Read> Read for Fault<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.storage.read(bytes)
    }
}

impl<S: Write> Write for Fault<S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.storage.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.storage.flush()
    }
}

impl<S: Seek> Seek for Fault<S> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.storage.seek(position)
    }
}

impl<S: DurableStorage> DurableStorage for Fault<S> {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.storage.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        if self.control.sync.get() {
            return Err(IoError::Io(io::ErrorKind::Other));
        }
        self.storage.sync_storage()
    }
}

impl<S: ResizableDurableStorage> ResizableDurableStorage for Fault<S> {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        let Some(failure) = self.control.resize.get() else {
            return self.storage.resize_storage(length);
        };
        match failure {
            ResizeFault::Before => {}
            ResizeFault::Partial => {
                let current = self.storage.storage_len()?;
                assert!(current >= length);
                self.storage
                    .resize_storage(length + (current - length) / 2)?;
            }
            ResizeFault::After => self.storage.resize_storage(length)?,
        }
        Err(IoError::Io(io::ErrorKind::Other))
    }
}

#[test]
fn retained_v2_resize_and_sync_failures_recover_same_pair_without_inventing_an_outcome() {
    for (before, after) in [
        (b"old".as_slice(), b"replacement".as_slice()),
        (b"before-tail".as_slice(), b"new".as_slice()),
    ] {
        for failure in 0..5 {
            let (_directory, path, mut pair) = pair(before);
            let target_control = Rc::new(Control::default());
            let journal_control = Rc::new(Control::default());
            {
                let (target, journal) = pair.resizable_storage().unwrap();
                let mut target = Fault::new(target, target_control.clone());
                let mut journal = Fault::new(journal, journal_control.clone());
                let prepared = prepare_resizable_expected(
                    &mut target,
                    &mut journal,
                    before,
                    after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                // Arm only after complete journal preparation; each fault lies
                // on the actual paired commit path, not a synthetic state loop.
                match failure {
                    0 => target_control.resize.set(Some(ResizeFault::Before)),
                    1 => target_control.resize.set(Some(ResizeFault::Partial)),
                    2 => target_control.resize.set(Some(ResizeFault::After)),
                    3 => target_control.sync.set(true),
                    4 => journal_control.sync.set(true),
                    _ => unreachable!(),
                }
                assert!(prepared.commit(&mut budget(), &|| false).is_err());
            }
            assert_eq!(pair.known_outcome(), None);
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            // Failure may have changed bytes/length or persisted a complete
            // marker. Only checked replay establishes its current outcome.
            let expected = if failure == 4 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            for _ in 0..2 {
                let (mut target, mut journal) = pair.resizable_storage().unwrap();
                let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
                assert_eq!(
                    checked
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    expected
                );
            }
            assert_eq!(
                fs::read(&path).unwrap(),
                if failure == 4 { after } else { before }
            );
            pair.retire(expected).unwrap();
            assert_eq!(pair.known_outcome(), Some(expected));
            pair.cleanup_retired().unwrap();
            let mut target = pair.into_target_after_clean().unwrap();
            assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
            target.release().unwrap();
        }
    }
}

#[test]
fn retired_v2_garbage_failure_retains_committed_outcome_and_target_lease() {
    let (directory, path, mut pair) = pair(b"old");
    {
        let (mut target, mut journal) = pair.resizable_storage().unwrap();
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            b"old",
            b"replacement",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    }
    pair.retire(RecoveryOutcome::Committed).unwrap();
    let retired = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|entry| entry.is_dir())
        .unwrap();
    // A controlled unexpected entry prevents directory removal. Production
    // cleanup must retain known completion instead of replaying the journal.
    fs::write(retired.join("obstacle"), b"unrelated").unwrap();
    let failure = pair.cleanup_retired().unwrap_err();
    assert_eq!(failure.outcome, Some(RecoveryOutcome::Committed));
    assert_eq!(pair.known_outcome(), Some(RecoveryOutcome::Committed));
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    assert!(matches!(
        pair.resizable_storage(),
        Err(PairError::InvalidState)
    ));
    assert!(matches!(SharedFile::open(&path), Err(IoError::LockBusy)));
    let repeated = pair.cleanup_retired().unwrap_err();
    assert_eq!(repeated.outcome, Some(RecoveryOutcome::Committed));
    pair.release().unwrap();
}
