use super::{MemoryStorage, budget};
use crate::{
    IoError,
    journal::{DurableStorage, JournalError, RecoveryOutcome, prepare, recover},
};
use std::{
    cell::RefCell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

#[derive(Default)]
struct Control {
    calls: usize,
    fail_at: usize,
    after_effect: bool,
    hit: bool,
    events: Vec<(&'static str, &'static str)>,
}

struct Device {
    bytes: Cursor<Vec<u8>>,
    durable: Vec<u8>,
    name: &'static str,
    control: Rc<RefCell<Control>>,
}

impl Device {
    fn new(bytes: &[u8], name: &'static str, control: Rc<RefCell<Control>>) -> Self {
        Self {
            bytes: Cursor::new(bytes.to_vec()),
            durable: bytes.to_vec(),
            name,
            control,
        }
    }

    fn before(&mut self, operation: &'static str) -> io::Result<bool> {
        let mut control = self.control.borrow_mut();
        control.calls += 1;
        control.events.push((self.name, operation));
        let fails = control.calls == control.fail_at;
        if fails {
            control.hit = true;
            if !control.after_effect {
                return Err(io::ErrorKind::Other.into());
            }
        }
        Ok(fails)
    }

    fn after<T>(&self, fails: bool, result: T) -> io::Result<T> {
        if fails {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(result)
        }
    }
}

impl Read for Device {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let fails = self.before("read")?;
        let count = self.bytes.read(bytes)?;
        self.after(fails, count)
    }
}

impl Write for Device {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let fails = self.before("write")?;
        // Force repeated short writes, including partial commit markers.
        let count = self.bytes.write(&bytes[..bytes.len().min(3)])?;
        self.after(fails, count)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush is not a durability barrier")
    }
}

impl Seek for Device {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let fails = self.before("seek")?;
        let position = self.bytes.seek(position)?;
        self.after(fails, position)
    }
}

impl DurableStorage for Device {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        let fails = self.before("length")?;
        Ok(self.after(fails, self.bytes.get_ref().len() as u64)?)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        let fails = self.before("sync")?;
        self.durable.clone_from(self.bytes.get_ref());
        Ok(self.after(fails, ())?)
    }
}

fn attempt(target: &mut Device, journal: &mut Device) {
    if let Ok(prepared) = prepare(target, journal, b"modified", &mut budget(), &|| false) {
        let _ = prepared.commit(&mut budget(), &|| false);
    }
}

#[test]
fn commit_barriers_precede_target_changes_and_commit_markers() {
    let control = Rc::new(RefCell::new(Control::default()));
    let mut target = Device::new(b"original", "target", control.clone());
    let mut journal = Device::new(b"", "journal", control.clone());
    prepare(
        &mut target,
        &mut journal,
        b"modified",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .commit(&mut budget(), &|| false)
    .unwrap();

    let events = &control.borrow().events;
    let journal_sync = events
        .iter()
        .position(|e| *e == ("journal", "sync"))
        .unwrap();
    let first_target_write = events
        .iter()
        .position(|e| *e == ("target", "write"))
        .unwrap();
    let target_sync = events
        .iter()
        .position(|e| *e == ("target", "sync"))
        .unwrap();
    let marker_write = events
        .iter()
        .enumerate()
        .find(|(i, e)| *i > target_sync && **e == ("journal", "write"))
        .unwrap()
        .0;
    let final_sync = events
        .iter()
        .rposition(|e| *e == ("journal", "sync"))
        .unwrap();

    assert!(journal_sync < first_target_write && first_target_write < target_sync);
    assert!(target_sync < marker_write && marker_write < final_sync);
    assert_eq!(target.durable, b"modified");
}

#[test]
fn every_commit_callback_failure_recovers_an_entire_old_or_new_image() {
    let baseline = Rc::new(RefCell::new(Control::default()));
    attempt(
        &mut Device::new(b"original", "target", baseline.clone()),
        &mut Device::new(b"", "journal", baseline.clone()),
    );
    let calls = baseline.borrow().calls;

    for fail_at in 1..=calls {
        for after_effect in [false, true] {
            let control = Rc::new(RefCell::new(Control {
                fail_at,
                after_effect,
                ..Default::default()
            }));
            let mut target = Device::new(b"original", "target", control.clone());
            let mut journal = Device::new(b"", "journal", control.clone());
            attempt(&mut target, &mut journal);
            assert!(control.borrow().hit);

            // Unsynchronized writes may either survive or disappear on crash.
            // Try both for each device, while respecting completed sync barriers.
            for target_image in [target.bytes.get_ref(), &target.durable] {
                for journal_image in [journal.bytes.get_ref(), &journal.durable] {
                    let mut target = MemoryStorage(Cursor::new(target_image.clone()));
                    let mut journal = MemoryStorage(Cursor::new(journal_image.clone()));
                    let result = recover(&mut target, &mut journal, &mut budget(), &|| false);
                    match result {
                        Ok(RecoveryOutcome::Committed) => {
                            assert_eq!(target.0.get_ref(), b"modified")
                        }
                        Ok(RecoveryOutcome::RolledBack) => {
                            assert_eq!(target.0.get_ref(), b"original")
                        }
                        Err(JournalError::InvalidJournal) => {
                            assert_eq!(target.0.get_ref(), b"original")
                        }
                        other => panic!("unexpected recovery at callback {fail_at}: {other:?}"),
                    }
                }
            }
        }
    }
}

#[test]
fn every_recovery_callback_failure_can_be_recovered_again() {
    let mut original = MemoryStorage(Cursor::new(b"original".to_vec()));
    let mut prepared = MemoryStorage(Cursor::new(Vec::new()));
    prepare(
        &mut original,
        &mut prepared,
        b"modified",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .commit(&mut budget(), &|| false)
    .unwrap();
    // A torn commit marker requires rollback, even if all target bytes changed.
    let length = prepared.0.get_ref().len();
    prepared.0.get_mut().truncate(length - 7);
    let journal_bytes = prepared.0.into_inner();

    let baseline = Rc::new(RefCell::new(Control::default()));
    recover(
        &mut Device::new(b"modified", "target", baseline.clone()),
        &mut Device::new(&journal_bytes, "journal", baseline.clone()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let calls = baseline.borrow().calls;

    for fail_at in 1..=calls {
        for after_effect in [false, true] {
            let control = Rc::new(RefCell::new(Control {
                fail_at,
                after_effect,
                ..Default::default()
            }));
            let mut target = Device::new(b"modified", "target", control.clone());
            let mut journal = Device::new(&journal_bytes, "journal", control.clone());
            assert!(recover(&mut target, &mut journal, &mut budget(), &|| false).is_err());

            for image in [target.bytes.get_ref(), &target.durable] {
                let mut target = MemoryStorage(Cursor::new(image.clone()));
                let mut journal = MemoryStorage(Cursor::new(journal.bytes.get_ref().clone()));
                assert_eq!(
                    recover(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
                    RecoveryOutcome::RolledBack
                );
                assert_eq!(target.0.get_ref(), b"original");
            }
        }
    }
}
