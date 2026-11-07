//! Existing Windows targets retain both the exact file and its parent handles.
use crate::journal::filesystem_contracts::TargetLockState;
use crate::{
    IoError,
    progress::check,
    windows_fs::{self as filesystem, FileIdentity, Location},
};
use std::{
    fs::File,
    io::{self, ErrorKind, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// An existing regular local-NTFS target held with exclusive Windows sharing.
/// No writable, cloned or raw handle is exposed. The relative open establishes
/// association with the captured parent. Denying delete sharing on both handles
/// pins that association until actual handle release, without a second path open.
pub struct OfflineTarget {
    parent: Option<File>,
    parent_identity: FileIdentity,
    identity: FileIdentity,
    file: Option<File>,
    state: TargetLockState,
}

impl OfflineTarget {
    /// Open the existing child without creating, truncating or following a final
    /// reparse point. Only qualified local NTFS storage is admitted.
    pub fn open(
        path: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        let _span = tracing::debug_span!("io.journal.open_offline_target").entered();
        let mut location = Location::capture(path, budget, cancelled)?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, location.name.len() as u64 * 16 + 8)?;
        let file = filesystem::open_existing_write_through(&location.parent, &mut location.name)?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        let identity = filesystem::identity(&file)?;
        check(budget, cancelled)?;
        Ok(Self {
            parent: Some(location.parent),
            parent_identity: location.identity,
            identity,
            file: Some(file),
            state: TargetLockState::Held,
        })
    }

    pub const fn lock_state(&self) -> TargetLockState {
        self.state
    }

    pub fn length(&self) -> Result<u64, IoError> {
        Ok(self.file()?.metadata()?.len())
    }

    /// Acknowledge actual close of both file and parent. Failure retains any
    /// remaining owner, denies subsequent I/O and forbids automatic release retry.
    /// Dropping after failure is best effort and is not an acknowledged release.
    pub fn release(&mut self) -> Result<(), IoError> {
        self.require_held()?;
        self.state = TargetLockState::ReleaseUncertain;
        filesystem::close_file(&mut self.file)?;
        filesystem::close_file(&mut self.parent)?;
        self.state = TargetLockState::Released;
        Ok(())
    }

    fn require_held(&self) -> Result<(), IoError> {
        if self.state != TargetLockState::Held {
            return Err(IoError::Io(ErrorKind::PermissionDenied));
        }
        Ok(())
    }

    fn file(&self) -> Result<&File, IoError> {
        self.require_held()?;
        self.file
            .as_ref()
            .ok_or(IoError::Io(ErrorKind::PermissionDenied))
    }

    pub(super) fn file_mut(&mut self) -> Result<&mut File, IoError> {
        self.require_held()?;
        self.file
            .as_mut()
            .ok_or(IoError::Io(ErrorKind::PermissionDenied))
    }

    pub(super) fn parent(&self) -> Result<&File, IoError> {
        self.require_held()?;
        self.parent
            .as_ref()
            .ok_or(IoError::Io(ErrorKind::PermissionDenied))
    }

    pub(super) fn identity(&self) -> FileIdentity {
        self.identity.clone()
    }

    pub(super) fn parent_identity(&self) -> FileIdentity {
        self.parent_identity.clone()
    }

    pub(super) fn require_entry(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        self.require_held()?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 2)?;
        if filesystem::identity(self.file()?)? != self.identity
            || filesystem::identity(self.parent()?)? != self.parent_identity
        {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        check(budget, cancelled)
    }

    pub(super) fn sync_baseline(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        self.require_entry(budget, cancelled)?;
        check(budget, cancelled)?;
        filesystem::flush(self.file()?)?;
        check(budget, cancelled)
    }
}

impl Read for OfflineTarget {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.file_mut()
            .map_err(|_| io::Error::from(ErrorKind::PermissionDenied))?
            .read(output)
    }
}

impl Seek for OfflineTarget {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file_mut()
            .map_err(|_| io::Error::from(ErrorKind::PermissionDenied))?
            .seek(position)
    }
}

#[cfg(test)]
mod tests;
