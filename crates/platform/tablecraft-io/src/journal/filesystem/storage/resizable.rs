//! An explicit resize capability borrowing only the retained paired target.
use super::PairedStorage;
use crate::{
    IoError,
    journal::{DurableStorage, ResizableDurableStorage},
};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};

/// Resizable view issued only by an active OfflineJournalPair target borrow.
/// It retains that exact locked descriptor. No raw handle, clone, unlock,
/// target replacement or journal-resize capability is exposed.
pub struct ResizablePairedTarget<'a> {
    storage: PairedStorage<'a>,
}

impl<'a> ResizablePairedTarget<'a> {
    pub(in crate::journal::filesystem) fn new(storage: PairedStorage<'a>) -> Self {
        Self { storage }
    }
}

impl Read for ResizablePairedTarget<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.storage.read(bytes)
    }
}

impl Write for ResizablePairedTarget<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.storage.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.storage.flush()
    }
}

impl Seek for ResizablePairedTarget<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.storage.seek(position)
    }
}

impl DurableStorage for ResizablePairedTarget<'_> {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.storage.storage_len()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.storage.sync_storage()
    }
}

impl ResizableDurableStorage for ResizablePairedTarget<'_> {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        if !self.storage.writable {
            return Err(IoError::Io(ErrorKind::PermissionDenied));
        }
        self.storage.file.resize_storage(length)
    }
}

#[cfg(test)]
mod tests;
