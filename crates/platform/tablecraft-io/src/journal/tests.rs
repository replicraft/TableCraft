use super::*;
use crate::IoError;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_core::limits::ResourceBudget;

mod faults;
mod validation;

struct MemoryStorage(Cursor<Vec<u8>>);

impl Read for MemoryStorage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}

impl Write for MemoryStorage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for MemoryStorage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

impl DurableStorage for MemoryStorage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.0.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 1_000_000)
}

#[test]
fn prepared_images_support_explicit_commit_rollback_and_repeated_recovery() {
    for commit in [false, true] {
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
        let expected = if commit {
            prepared.commit(&mut budget(), &|| false).unwrap();
            RecoveryOutcome::Committed
        } else {
            prepared.rollback(&mut budget(), &|| false).unwrap()
        };

        for _ in 0..2 {
            assert_eq!(
                recover(&mut target, &mut journal, &mut budget(), &|| false).unwrap(),
                expected
            );
        }

        assert_eq!(
            target.0.get_ref().as_slice(),
            if commit { b"modified" } else { b"original" }
        );
        assert!(!journal.0.get_ref().is_empty());
    }
}
