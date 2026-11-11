//! Borrowed storage-neutral row and scalar collection views.
mod collection;
mod projection;
mod recordset;

pub use collection::MultivalueList;
pub use projection::{ProjectedRow, ProjectionError, validate_projection};

pub use recordset::{
    MemberAvailability, OwnedRecordset, RecordsetError, RecordsetPresence, RecordsetSchema,
    StoredCell,
};
