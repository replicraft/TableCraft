//! Shared inspected-action facts; UPDATE, DELETE and INSERT retain distinct capabilities.
use std::path::PathBuf;

pub(super) struct QueryActionTarget {
    pub(super) path: PathBuf,
    pub(super) document_revision: u64,
    pub(super) table: String,
}
