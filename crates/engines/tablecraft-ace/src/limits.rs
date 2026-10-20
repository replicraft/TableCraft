//! Structural ceilings for the current bounded reader, not format maxima.
// Addressable read-source envelope; bounded operations need not retain the file.
pub(crate) const MAX_READ_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
// Complete capture and mutation paths retain a separate memory ceiling.
pub(crate) const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_PAGE_READS: u32 = 4096;
pub(crate) const MAX_MAPPED_PAGES: usize = 4096;
pub(crate) const MAX_RELOCATION_HOPS: usize = 16;
pub(crate) const MAX_COLUMNS: usize = 256;
pub(crate) const MAX_INDEXES: usize = 256;
pub(crate) const MAX_LIVE_ROWS: usize = 16_384;
pub(crate) const MAX_ENCODED_TEXT_BYTES: usize = 2048;

pub(crate) const MAX_LONG_TEXT_BYTES: usize = 65_536;
pub(crate) const MAX_LONG_VALUE_CHUNKS: usize = 128;
pub(crate) const MAX_LONG_VALUE_BYTES: usize = 65_536;
pub(crate) const MAX_INDEX_DEPTH: usize = 16;
pub(crate) const MAX_DEFINITION_PAGES: usize = 64;
