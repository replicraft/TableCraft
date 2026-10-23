//! Consumer snapshot ceilings are separate from native geometry and read budgets.
use super::TablePageRequest;
use crate::InspectError;

/// Checked complete-table retention limits and internal traversal batch size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableSnapshotLimits {
    maximum_rows: u32,
    batch_rows: u32,
}
impl TableSnapshotLimits {
    /// Largest explicit complete table in this consumer profile, not a format limit.
    pub const MAX_ROWS: u32 = 1_000_000;
    /// Construct positive bounded row and batch ceilings without starting I/O.
    pub fn new(maximum_rows: u32, batch_rows: u32) -> Result<Self, InspectError> {
        if maximum_rows == 0
            || maximum_rows > Self::MAX_ROWS
            || batch_rows == 0
            || batch_rows > TablePageRequest::MAX_ROWS
        {
            return Err(InspectError::InvalidSnapshotLimits);
        }
        Ok(Self {
            maximum_rows,
            batch_rows,
        })
    }
    /// Maximum complete snapshot rows; metadata rows additionally consume read budgets.
    pub const fn maximum_rows(self) -> u32 {
        self.maximum_rows
    }
    /// Maximum records decoded by each fetch.
    pub const fn batch_rows(self) -> u32 {
        self.batch_rows
    }
}
impl Default for TableSnapshotLimits {
    fn default() -> Self {
        Self {
            maximum_rows: TablePageRequest::MAX_ROWS,
            batch_rows: 256,
        }
    }
}
