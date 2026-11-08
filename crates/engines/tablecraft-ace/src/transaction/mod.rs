//! Offline snapshots and optional journaled native writes.
#[cfg(feature = "journal")]
mod intent;
#[cfg(feature = "journal")]
pub use intent::{
    NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates, NativeFixedEditIntent,
    NativeFixedIntentGroup, NativeInt32EditIntent, NativeIntentError, NativeUpdateBody,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, RecoverableNativeEditPlan,
};
#[cfg(feature = "journal")]
mod native_recovery;
#[cfg(feature = "journal")]
mod update_rebuild;
#[cfg(feature = "journal")]
pub use native_recovery::{QualifiedNativeRecovery, qualify_native_update_recovery};
#[cfg(feature = "journal")]
mod qualified_recovery;
#[cfg(feature = "journal")]
pub use qualified_recovery::{
    NativeRecoveryQualificationError, QualifiedJet4Recovery, qualify_jet4_int32_recovery,
};
#[cfg(feature = "journal")]
mod resizable_write;
#[cfg(feature = "journal")]
mod write;
#[cfg(feature = "journal")]
pub use resizable_write::{PreparedNativeUpdate, PreparedResizableNativeEdit};
#[cfg(feature = "journal")]
mod allocation_recovery;
use crate::{Header, InspectError, header, inspect_header};
#[cfg(feature = "journal")]
pub use allocation_recovery::{
    QualifiedNativeAllocationRecovery, qualify_native_allocation_recovery,
};
use std::{
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{SharedFile, read_exact_at};
#[cfg(feature = "journal")]
pub use write::{NativeRecoveryError, PreparedNativeEdit, recover_native_edit};

/// An immutable, bounded database image captured under a shared OS file lock.
/// Complete capture is capped at 64 MiB even when the read-source envelope and
/// caller budget permit a larger file.
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
        // Read sources can exceed the complete-image ceiling. Check before any
        // image allocation, including after an uncooperating process resized it.
        if length > crate::limits::MAX_IMAGE_BYTES {
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

#[cfg(feature = "journal")]
pub use intent::{
    NativeDeleteIntent, QualifiedNativeDeleteRecovery, RecoverableNativeDeletePlan,
    qualify_native_delete_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeInsertIntent, QualifiedNativeInsertRecovery, RecoverableNativeInsertPlan,
    qualify_native_insert_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeOmittedInsertIntent, QualifiedNativeOmittedInsertRecovery,
    RecoverableNativeOmittedInsertPlan, qualify_native_omitted_insert_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeEofInsertIntent, QualifiedNativeEofInsertRecovery, RecoverableNativeEofInsertPlan,
    RecoverableNativeInsertOperation, qualify_native_eof_insert_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeCompleteInsertIntent, QualifiedNativeCompleteInsertRecovery,
    RecoverableNativeCompleteInsertPlan, qualify_native_complete_insert_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeClockInsertIntent, QualifiedNativeClockInsertRecovery, RecoverableNativeClockInsertPlan,
    qualify_native_clock_insert_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use intent::{
    FirstTableIntentError, NativeFirstTableIntent, PreparedFirstTable, QualifiedFirstTableRecovery,
    qualify_native_first_table_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use intent::{
    AddTableIntentError, NativeAddTableIntent, PreparedAddTable, QualifiedAddTableRecovery,
    qualify_native_add_table_recovery,
};

#[cfg(feature = "journal")]
pub use intent::{
    NativeTableRenameIntent, PreparedTableRename, QualifiedTableRenameRecovery,
    RecoverableNativeTableRenamePlan, RecoverableTableRenameError, TableRenameIntentError,
    qualify_native_table_rename_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use intent::{
    AddColumnIntentError, NativeAddColumnIntent, QualifiedAddColumnRecovery,
    RecoverableAddColumnPlan, qualify_native_add_column_recovery,
};
