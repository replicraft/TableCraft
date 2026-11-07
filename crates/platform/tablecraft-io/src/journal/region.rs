//! A logical journal stream that cannot address its host-owned physical prefix.
use super::DurableStorage;
use crate::{IoError, progress::check, seek_absolute};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};
use tablecraft_core::limits::ResourceBudget;

/// An exclusive borrowed journal region after an immutable host-owned prefix.
/// This supplies byte bounds, not file identity, pairing or namespace durability.
/// The host must exclude other mutation of the underlying storage while borrowed.
/// It is deliberately not resizable: V2 journals also append to ordinary storage.
/// Length queries, flushes and synchronization invalidate the cached cursor;
/// callers must explicitly seek from Start before subsequent transfers. Other
/// transfer failures require both a fresh length query and an absolute seek.
pub struct BoundedJournalRegion<'a, S: DurableStorage + ?Sized> {
    storage: &'a mut S,
    prefix: u64,
    maximum: u64,
    position: Option<u64>,
    length: Option<u64>,
}

impl<'a, S: DurableStorage + ?Sized> BoundedJournalRegion<'a, S> {
    /// A paired owner already holds the prefix contract. Issuing this view does
    /// no I/O: the image protocol must inspect actual length and seek explicitly
    /// under its own budget before any transfer can proceed.
    #[cfg(any(windows, test))]
    pub(in crate::journal) fn deferred(
        storage: &'a mut S,
        prefix: u64,
        maximum: u64,
    ) -> Result<Self, IoError> {
        prefix
            .checked_add(maximum)
            .ok_or(IoError::Io(ErrorKind::InvalidInput))?;
        Ok(Self {
            storage,
            prefix,
            maximum,
            position: None,
            length: None,
        })
    }

    /// Validate existing physical length and establish logical cursor zero.
    /// No prefix bytes are read or written. Each setup callback remains under
    /// caller work and cancellation controls; callbacks own their own timeouts.
    pub fn new(
        storage: &'a mut S,
        prefix: u64,
        maximum: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        check(budget, cancelled)?;
        let end = prefix
            .checked_add(maximum)
            .ok_or(IoError::Io(ErrorKind::InvalidInput))?;
        let physical = storage.storage_len()?;
        check(budget, cancelled)?;
        if !(prefix..=end).contains(&physical) {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        seek_absolute(storage, prefix, budget, cancelled)?;
        Ok(Self {
            storage,
            prefix,
            maximum,
            position: Some(0),
            length: Some(physical - prefix),
        })
    }

    fn known(&self) -> io::Result<(u64, u64)> {
        self.position
            .zip(self.length)
            .ok_or_else(|| io::Error::from(ErrorKind::InvalidData))
    }

    fn uncertain(&mut self) {
        self.position = None;
        self.length = None;
    }
}

impl<S: DurableStorage + ?Sized> Read for BoundedJournalRegion<'_, S> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let (position, length) = self.known()?;
        let available = length.saturating_sub(position);
        let count = output
            .len()
            .min(usize::try_from(available).unwrap_or(usize::MAX));
        if count == 0 {
            return Ok(0);
        }
        match self.storage.read(&mut output[..count]) {
            Ok(read) if read <= count => {
                self.position = Some(position + read as u64);
                Ok(read)
            }
            Ok(_) => {
                self.uncertain();
                Err(ErrorKind::InvalidData.into())
            }
            Err(error) => {
                // std::io errors report no transferred bytes. Interrupted is
                // retryable under that contract; other errors require recovery.
                if error.kind() != ErrorKind::Interrupted {
                    self.uncertain();
                }
                Err(error)
            }
        }
    }
}

impl<S: DurableStorage + ?Sized> Write for BoundedJournalRegion<'_, S> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let (position, length) = self.known()?;
        if position > length || input.len() as u64 > self.maximum - position {
            return Err(ErrorKind::InvalidInput.into());
        }
        if input.is_empty() {
            return Ok(0);
        }
        match self.storage.write(input) {
            Ok(written) if written <= input.len() => {
                let end = position + written as u64;
                self.position = Some(end);
                self.length = Some(length.max(end));
                Ok(written)
            }
            Ok(_) => {
                self.uncertain();
                Err(ErrorKind::InvalidData.into())
            }
            Err(error) => {
                if error.kind() != ErrorKind::Interrupted {
                    self.uncertain();
                }
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.position = None;
        match self.storage.flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.length = None;
                Err(error)
            }
        }
    }
}

impl<S: DurableStorage + ?Sized> Seek for BoundedJournalRegion<'_, S> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let logical = match position {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self
                .position
                .and_then(|base| base.checked_add_signed(delta)),
            SeekFrom::End(delta) => self.length.and_then(|base| base.checked_add_signed(delta)),
        }
        .filter(|position| *position <= self.maximum)
        .ok_or_else(|| io::Error::from(ErrorKind::InvalidInput))?;

        self.position = None;
        let actual = self.storage.seek(SeekFrom::Start(self.prefix + logical))?;
        if actual != self.prefix + logical {
            return Err(ErrorKind::InvalidData.into());
        }
        self.position = Some(logical);
        Ok(logical)
    }
}

impl<S: DurableStorage + ?Sized> DurableStorage for BoundedJournalRegion<'_, S> {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        // The underlying length callback need not preserve its cursor. A fresh
        // absolute seek must establish it again before subsequent transfers.
        self.position = None;
        self.length = None;
        let physical = self.storage.storage_len()?;
        let logical = physical
            .checked_sub(self.prefix)
            .filter(|length| *length <= self.maximum)
            .ok_or(IoError::Io(ErrorKind::InvalidData))?;
        self.length = Some(logical);
        Ok(logical)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.position = None;
        match self.storage.sync_storage() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.length = None;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests;
