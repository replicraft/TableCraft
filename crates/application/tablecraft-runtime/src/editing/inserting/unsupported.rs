//! Unsupported hosts retain display facts without native insertion authority.
use super::{InsertRetryWitness, InsertTarget};
use std::{path::PathBuf, sync::Arc};

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: InsertTarget,
    pub(in crate::editing) retry: Arc<InsertRetryWitness>,
}
