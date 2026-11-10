//! Retained payload envelopes for qualified mutation producers and independent proofs.
// These are current writer admission boundaries, not native read-format maxima.
pub(in crate::table::mutation) const QUALIFIED_LONG_TEXT_BYTES: usize = 1024 * 1024;
pub(in crate::table::mutation) const QUALIFIED_LONG_VALUE_BYTES: usize = 2 * 1024 * 1024;
pub(in crate::table::mutation) const QUALIFIED_LONG_VALUE_CHUNKS: usize = 1024;
