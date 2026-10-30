//! Actual held-relative namespace primitives shared by publication consumers.
use crate::{IoError, progress::check};
use rustix::fs::{self, AtFlags, Mode, OFlags};
use std::ffi::CStr;
use std::fs::{File, Metadata};
use std::io::ErrorKind;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

impl ObjectIdentity {
    pub(crate) fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

struct NulBuffer(Vec<u8>);

impl NulBuffer {
    fn capture(bytes: &[u8], budget: &mut ResourceBudget) -> Result<Self, IoError> {
        let length = bytes
            .len()
            .checked_add(1)
            .ok_or(IoError::Io(ErrorKind::InvalidInput))?;
        budget.charge(Resource::Bytes, length as u64)?;
        budget.charge(Resource::Steps, length as u64)?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(length)
            .map_err(|_| IoError::Io(ErrorKind::OutOfMemory))?;
        buffer.extend_from_slice(bytes);
        buffer.push(0);
        Ok(Self(buffer))
    }

    fn as_c_str(&self) -> &CStr {
        CStr::from_bytes_with_nul(&self.0).expect("capture validated NUL-terminated path")
    }
}

pub(crate) struct RelativeName(NulBuffer);

impl RelativeName {
    pub(crate) fn as_c_str(&self) -> &CStr {
        self.0.as_c_str()
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0.0[..self.0.0.len() - 1]
    }

    pub(crate) fn validation_work(&self) -> u64 {
        self.0.0.len() as u64
    }
}

pub(crate) struct HeldDirectory {
    file: File,
    identity: ObjectIdentity,
}

impl HeldDirectory {
    // The actual caller has already observed this descriptor's metadata. Keep
    // that same observation and callback ordering rather than reopening by name.
    pub(crate) fn adopt_observed(file: File, metadata: &Metadata) -> Result<Self, IoError> {
        if !metadata.is_dir() {
            return Err(IoError::Io(ErrorKind::InvalidInput));
        }
        Ok(Self {
            file,
            identity: ObjectIdentity::from_metadata(metadata),
        })
    }

    pub(crate) fn identity(&self) -> ObjectIdentity {
        self.identity
    }

    pub(crate) fn verify_child(
        &self,
        name: &CStr,
        expected: ObjectIdentity,
    ) -> Result<(), IoError> {
        let stat =
            fs::statat(self, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if stat.st_dev as u64 != expected.device || stat.st_ino as u64 != expected.inode {
            return Err(IoError::Io(ErrorKind::InvalidData));
        }
        Ok(())
    }

    pub(crate) fn sync(&self) -> Result<(), IoError> {
        self.file.sync_all().map_err(IoError::from)
    }
}

impl AsFd for HeldDirectory {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

pub(crate) struct Location {
    parent: HeldDirectory,
    name: RelativeName,
}

impl Location {
    pub(crate) fn capture(
        output: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, IoError> {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, output.as_os_str().len() as u64)?;
        // Components normalizes trailing '/.'; validate the actual byte spelling.
        let raw = output.as_os_str().as_bytes();
        let component = raw.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
        if component.is_empty() || component == b"." || component == b".." || raw.contains(&0) {
            return Err(IoError::Io(ErrorKind::InvalidInput));
        }
        let name = RelativeName(NulBuffer::capture(component, budget)?);
        let parent_path = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent_buffer = NulBuffer::capture(parent_path.as_os_str().as_bytes(), budget)?;
        check(budget, cancelled)?;
        let file = File::from(
            fs::openat(
                fs::CWD,
                parent_buffer.as_c_str(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?,
        );
        budget.charge(Resource::Steps, 1)?;
        let metadata = file.metadata()?;
        let parent = HeldDirectory::adopt_observed(file, &metadata)?;
        Ok(Self { parent, name })
    }

    pub(crate) fn into_parts(self) -> (HeldDirectory, RelativeName) {
        (self.parent, self.name)
    }
}
