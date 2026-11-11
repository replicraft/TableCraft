//! Backend-neutral owned snapshots and bounded edit planning contracts.
//! No concrete engines, adapters, SQL syntax or application hosts are imported.
#![deny(missing_docs)]

mod editing;
mod error;
mod snapshot;
mod views;

pub use editing::{
    CellUpdate, ColumnUpdates, CompleteInsertableBatch, DeletableTable, EditableBatch,
    EditableTable, InsertValue, InsertableTable, OmittingInsertableBatch, OmittingInsertableTable,
    UpdateProfile,
};
pub use error::SnapshotError;
pub use snapshot::{RowHandle, Snapshot};
pub use views::{
    MemberAvailability, MultivalueList, OwnedRecordset, ProjectedRow, ProjectionError,
    RecordsetError, RecordsetKind, RecordsetPresence, RecordsetSchema, StoredCell,
    validate_projection,
};
