//! Unsupported hosts retain target facts without native edit authority.
use super::EditTarget;
use std::path::PathBuf;

pub(in crate::editing) struct ReviewedState {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: EditTarget,
}
