//! Backend-neutral owned snapshots and bounded edit planning contracts.
//! No concrete engines, adapters, SQL syntax or application hosts are imported.
#![deny(missing_docs)]

mod editing;
mod error;
mod snapshot;

pub use editing::{CellUpdate, EditableTable, UpdateProfile};
pub use error::SnapshotError;
pub use snapshot::{RowHandle, Snapshot};
