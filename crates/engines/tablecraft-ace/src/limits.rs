//! Structural ceilings for the current bounded reader, not format maxima.
// Addressable read-source envelope; bounded operations need not retain the file.
pub(crate) const MAX_READ_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
// Complete capture and mutation paths retain a separate memory ceiling.
pub(crate) const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
// Qualified physical mutation proofs retain their existing traversal scope.
// Ordinary read traversal uses source geometry and a paid cycle-detection set.
pub(crate) const MAX_RELOCATION_HOPS: usize = 16;
pub(crate) const MAX_COLUMNS: usize = 256;
// Access permits 255 active user fields; historical ordinal capacity is separate.
pub(crate) const MAX_USER_COLUMNS: usize = 255;
pub(crate) const MAX_INDEXES: usize = 256;
pub(crate) const MAX_LIVE_ROWS: usize = 16_384;

pub(crate) const MAX_INDEX_DEPTH: usize = 16;
