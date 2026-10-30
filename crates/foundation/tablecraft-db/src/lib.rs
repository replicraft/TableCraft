//! Backend-neutral owned snapshots and bounded edit planning contracts.
//! No concrete engines, adapters, SQL syntax or application hosts are imported.
#![deny(missing_docs)]

mod editing;
mod error;
mod snapshot;
mod views;

pub use editing::{CellUpdate, DeletableTable, EditableTable, UpdateProfile};
pub use error::SnapshotError;
pub use snapshot::{RowHandle, Snapshot};
pub use views::{MultivalueList, ProjectedRow, ProjectionError, validate_projection};
