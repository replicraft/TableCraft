//! Real memory consumers and independent complete-record authority.
mod generated;
pub(in crate::journal) mod goldens;
mod transactions;
mod validation;

use super::super::{DurableStorage, ResizableDurableStorage, tests::MemoryStorage};
use crate::IoError;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_core::limits::ResourceBudget;

impl ResizableDurableStorage for MemoryStorage {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        let length =
            usize::try_from(length).map_err(|_| IoError::Io(io::ErrorKind::InvalidInput))?;
        assert!(
            length <= self.0.get_ref().len(),
            "image must be written before resize"
        );
        self.0.get_mut().truncate(length);
        Ok(())
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 0, 32 * 1024 * 1024)
}

fn memory(bytes: &[u8]) -> MemoryStorage {
    MemoryStorage(Cursor::new(bytes.to_vec()))
}

struct Observed {
    storage: MemoryStorage,
    writes: usize,
    resizes: usize,
}

impl Observed {
    fn new(bytes: &[u8]) -> Self {
        Self {
            storage: memory(bytes),
            writes: 0,
            resizes: 0,
        }
    }
}

impl Read for Observed {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.storage.read(bytes)
    }
}

impl Write for Observed {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.storage.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush must not substitute for a durability barrier")
    }
}

impl Seek for Observed {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.storage.seek(position)
    }
}

impl DurableStorage for Observed {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.storage.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.storage.sync_storage()
    }
}

impl ResizableDurableStorage for Observed {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        self.resizes += 1;
        self.storage.resize_storage(length)
    }
}
