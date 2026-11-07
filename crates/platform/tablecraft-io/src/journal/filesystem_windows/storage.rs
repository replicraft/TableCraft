//! Borrow the same held target and a prefix-protected region of its sidecar.
use super::PairError;
use crate::{
    IoError,
    journal::{BoundedJournalRegion, DurableStorage, ResizableDurableStorage},
};
use std::{
    fs::File,
    io::{self, ErrorKind, Read, Seek, SeekFrom, Write},
};

pub(super) struct SidecarStorage {
    pub(super) file: Option<File>,
}

impl SidecarStorage {
    pub(super) fn file_mut(&mut self) -> Result<&mut File, IoError> {
        self.file
            .as_mut()
            .ok_or(IoError::Io(ErrorKind::PermissionDenied))
    }
}

impl Read for SidecarStorage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file_mut().map_err(io::Error::other)?.read(bytes)
    }
}

impl Write for SidecarStorage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file_mut().map_err(io::Error::other)?.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file_mut().map_err(io::Error::other)?.flush()
    }
}

impl Seek for SidecarStorage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file_mut().map_err(io::Error::other)?.seek(position)
    }
}

impl DurableStorage for SidecarStorage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.file_mut()?.metadata()?.len())
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.file_mut()?.sync_all().map_err(Into::into)
    }
}

enum Device<'a> {
    Target(&'a mut File),
    Journal(BoundedJournalRegion<'a, SidecarStorage>),
}

/// A borrowed device of this exact retained pair, never a replaceable handle.
pub struct PairedStorage<'a> {
    device: Device<'a>,
    writable: bool,
}

impl<'a> PairedStorage<'a> {
    pub(super) fn target(file: &'a mut File, writable: bool) -> Self {
        Self {
            device: Device::Target(file),
            writable,
        }
    }

    pub(super) fn journal(
        storage: &'a mut SidecarStorage,
        prefix: u64,
        maximum: u64,
        writable: bool,
    ) -> Result<Self, PairError> {
        Ok(Self {
            device: Device::Journal(BoundedJournalRegion::deferred(storage, prefix, maximum)?),
            writable,
        })
    }

    fn require_write(&self) -> io::Result<()> {
        if self.writable {
            Ok(())
        } else {
            Err(ErrorKind::PermissionDenied.into())
        }
    }
}

impl Read for PairedStorage<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match &mut self.device {
            Device::Target(file) => file.read(bytes),
            Device::Journal(region) => region.read(bytes),
        }
    }
}

impl Write for PairedStorage<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.require_write()?;
        match &mut self.device {
            Device::Target(file) => file.write(bytes),
            Device::Journal(region) => region.write(bytes),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.require_write()?;
        match &mut self.device {
            Device::Target(file) => file.flush(),
            Device::Journal(region) => region.flush(),
        }
    }
}

impl Seek for PairedStorage<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match &mut self.device {
            Device::Target(file) => file.seek(position),
            Device::Journal(region) => region.seek(position),
        }
    }
}

impl DurableStorage for PairedStorage<'_> {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        match &mut self.device {
            Device::Target(file) => Ok(file.metadata()?.len()),
            Device::Journal(region) => region.storage_len(),
        }
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        match &mut self.device {
            Device::Target(file) => file.sync_all().map_err(Into::into),
            Device::Journal(region) => region.sync_storage(),
        }
    }
}

/// Only the retained target has the native image-resize capability.
pub struct ResizablePairedTarget<'a> {
    storage: PairedStorage<'a>,
}

impl<'a> ResizablePairedTarget<'a> {
    pub(super) fn new(storage: PairedStorage<'a>) -> Self {
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
        self.storage.require_write().map_err(IoError::from)?;
        let Device::Target(file) = &mut self.storage.device else {
            return Err(IoError::Io(ErrorKind::PermissionDenied));
        };
        file.set_len(length).map_err(Into::into)
    }
}
