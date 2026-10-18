//! Offline read transactions; native write/shared-file transactions are unavailable.
use crate::{Header, InspectError, header, inspect_header};
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{SharedFile, read_exact_at};

/// An immutable, bounded database image captured under a shared OS file lock.
///
/// The caller must provide an offline file: this OS lock does not coordinate
/// Microsoft's ldb/laccdb protocol or stop noncooperating writes during capture.
/// After capture, reads use owned bytes; cooperating writers stay excluded until
/// drop. Each transaction owns its cursor and never writes or executes content.
/// It implements Read + Seek so existing inspection/query adapters can consume it.
///
/// ```compile_fail
/// use std::io::Write;
/// use tablecraft_ace::OfflineReadTransaction;
/// fn write_database(transaction: &mut OfflineReadTransaction) {
///     transaction.write_all(b"changed").unwrap();
/// }
/// ```
pub struct OfflineReadTransaction {
    image: Cursor<Vec<u8>>,
    header: Header,
    _lock: SharedFile,
}
impl OfflineReadTransaction {
    /// Capture one explicit offline file. Cancellation/work checks precede open;
    /// read bytes and retained image allocation both consume the caller's budget.
    /// Failure releases the lock and publishes no transaction or partial image.
    pub fn open(
        path: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        header::check(budget, cancelled)?;
        let mut file = SharedFile::open(path)?;
        let header = inspect_header(&mut file, budget, cancelled)?;
        let length = file.length()?;
        let size = usize::try_from(length).map_err(|_| InspectError::StructureLimit)?;
        // Header::parse already bounds native file size. Recheck the observed
        // length before allocation in case an uncooperating process resized it.
        if length > crate::limits::MAX_FILE_BYTES {
            return Err(InspectError::StructureLimit);
        }
        budget.charge_allocation(length, 1)?;
        budget.charge(Resource::Bytes, length)?;
        let mut image = Vec::new();
        image
            .try_reserve_exact(size)
            .map_err(|_| InspectError::Allocation)?;
        image.resize(size, 0);
        read_exact_at(&mut file, 0, &mut image, budget, cancelled)?;
        if file.length()? != length || Header::parse(&image, length)? != header {
            return Err(InspectError::Malformed);
        }
        header::check(budget, cancelled)?;
        Ok(Self {
            image: Cursor::new(image),
            header,
            _lock: file,
        })
    }
    /// Qualified header of the captured image, not a database integrity result.
    pub const fn header(&self) -> Header {
        self.header
    }
    /// End this read transaction and report OS unlock failure. Dropping a
    /// transaction instead performs best-effort release without a result.
    pub fn close(self) -> Result<(), InspectError> {
        self._lock.close().map_err(Into::into)
    }
}
impl Read for OfflineReadTransaction {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.image.read(bytes)
    }
}
impl Seek for OfflineReadTransaction {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.image.seek(position)
    }
}
