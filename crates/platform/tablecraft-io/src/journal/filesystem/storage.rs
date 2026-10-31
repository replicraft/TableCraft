use crate::{ExclusiveFile, IoError, journal::DurableStorage};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};

/// A borrowed storage view of an already paired locked descriptor. Its private
/// inner reference cannot be replaced with another ExclusiveFile guard. No raw
/// handle, clone, lock release or explicit truncation/resize method is exposed.
/// Ordinary Write/Seek behavior remains; hosts must use the qualified protocol.
pub struct PairedStorage<'a> {
    file: &'a mut ExclusiveFile,
    writable: bool,
}

impl<'a> PairedStorage<'a> {
    pub(super) fn new(file: &'a mut ExclusiveFile) -> Self {
        Self {
            file,
            writable: true,
        }
    }

    pub(super) fn confirmation(file: &'a mut ExclusiveFile) -> Self {
        Self {
            file,
            writable: false,
        }
    }
}

impl Read for PairedStorage<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}

impl Write for PairedStorage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.writable {
            return Err(ErrorKind::PermissionDenied.into());
        }
        self.file.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.writable {
            return Err(ErrorKind::PermissionDenied.into());
        }
        self.file.flush()
    }
}

impl Seek for PairedStorage<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}

impl DurableStorage for PairedStorage<'_> {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.file.length()
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.file.sync_all().map_err(IoError::from)
    }
}
