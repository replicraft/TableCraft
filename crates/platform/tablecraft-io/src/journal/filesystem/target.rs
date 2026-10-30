//! Retain one locked original file and its captured namespace without reopening.
use crate::{
    ExclusiveFile, IoError,
    namespace::{HeldDirectory, Location, ObjectIdentity, RelativeName},
    progress::check,
};
use rustix::fs::{self, Mode, OFlags};
use std::{
    fs::File,
    io::{self, ErrorKind, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Acknowledged lock state of this retained target, not native write permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetLockState {
    /// The retained descriptor acknowledged the exclusive lock acquisition.
    Held,
    /// Unlock returned an error; the descriptor survives, but exclusivity is unknown.
    ReleaseUncertain,
    /// Unlock acknowledged completion; no further read/storage access is offered.
    Released,
}

/// An existing regular target held exclusively under one captured directory.
/// Read and seek use the same descriptor that will enter the journal pair. No
/// raw/cloned handle or public Write implementation is exposed. Callers must
/// exclude noncooperating writers and namespace replacement outside this owner.
pub struct OfflineTarget {
    parent: HeldDirectory,
    name: RelativeName,
    identity: ObjectIdentity,
    file: ExclusiveFile,
    state: TargetLockState,
}

impl OfflineTarget {
    /// Capture a raw final component and acquire its existing file without
    /// following a final symlink, creating a file or truncating any bytes.
    pub fn open(
        path: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        let _span = tracing::debug_span!("io.journal.open_offline_target").entered();
        super::require_platform()?;

        let location = Location::capture(path, budget, cancelled)?;
        Self::open_location(location, budget, cancelled)
    }

    fn open_location(
        location: Location,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        let (parent, name) = location.into_parts();
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, name.validation_work())?;
        budget.charge(Resource::Steps, 1)?;
        let file = File::from(
            fs::openat(
                &parent,
                name.as_c_str(),
                OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|error| IoError::from(io::Error::from(error)))?,
        );

        // Adoption uses this already opened descriptor, with a regular-file
        // check and one nonblocking exclusive lock; it never opens the path.
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 2)?;
        let file = ExclusiveFile::from_owned(file)?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, 1)?;
        let identity = ObjectIdentity::from_metadata(&file.metadata()?);

        check(budget, cancelled)?;
        budget.charge(Resource::Steps, name.validation_work())?;
        budget.charge(Resource::Steps, 1)?;
        parent.verify_child(name.as_c_str(), identity)?;
        check(budget, cancelled)?;

        Ok(Self {
            parent,
            name,
            identity,
            file,
            state: TargetLockState::Held,
        })
    }

    /// Current acknowledged lock state; identity retention is separate from it.
    pub const fn lock_state(&self) -> TargetLockState {
        self.state
    }

    /// Length of the actual held file, available only while exclusivity is known.
    pub fn length(&self) -> Result<u64, IoError> {
        self.require_held()?;
        self.file.length()
    }

    /// Explicitly release without losing the retained object on an unlock error.
    /// A failed release denies further read/seek/storage operations. No automatic
    /// reacquisition or repeated unlock is performed; Drop remains best effort.
    pub fn release(&mut self) -> Result<(), IoError> {
        self.release_with(ExclusiveFile::try_release)
    }

    fn release_with(
        &mut self,
        release: impl FnOnce(&mut ExclusiveFile) -> Result<(), IoError>,
    ) -> Result<(), IoError> {
        self.require_held()?;
        self.state = TargetLockState::ReleaseUncertain;
        release(&mut self.file)?;
        self.state = TargetLockState::Released;
        Ok(())
    }

    fn require_held(&self) -> Result<(), IoError> {
        if self.state != TargetLockState::Held {
            return Err(IoError::Io(ErrorKind::PermissionDenied));
        }
        Ok(())
    }

    pub(super) fn identity(&self) -> ObjectIdentity {
        self.identity
    }

    pub(super) fn require_entry(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        self.require_held()?;
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, self.name.validation_work())?;
        budget.charge(Resource::Steps, 1)?;
        self.parent
            .verify_child(self.name.as_c_str(), self.identity)
    }

    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    pub(super) fn sync_baseline(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        self.require_entry(budget, cancelled)?;
        check(budget, cancelled)?;
        self.file.sync_all()?;
        check(budget, cancelled)?;
        self.parent.sync()?;
        check(budget, cancelled)
    }

    pub(super) fn storage_mut(&mut self) -> Result<&mut ExclusiveFile, IoError> {
        self.require_held()?;
        Ok(&mut self.file)
    }
}

impl Read for OfflineTarget {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.state != TargetLockState::Held {
            return Err(ErrorKind::PermissionDenied.into());
        }
        self.file.read(bytes)
    }
}

impl Seek for OfflineTarget {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.state != TargetLockState::Held {
            return Err(ErrorKind::PermissionDenied.into());
        }
        self.file.seek(position)
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
