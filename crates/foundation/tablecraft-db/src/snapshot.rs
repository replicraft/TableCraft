mod identity;
mod table;

pub use identity::{RowHandle, SnapshotId};
pub use table::Snapshot;

#[cfg(test)]
mod tests;
