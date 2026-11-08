//! Unsupported hosts retain display facts without native deletion authority.
use super::DeleteTarget;
use std::path::PathBuf;

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: DeleteTarget,
    pub(super) refresh_ordinal: u32,
}
