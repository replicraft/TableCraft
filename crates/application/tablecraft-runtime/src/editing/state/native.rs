//! Native editing state and document lifetime invalidation.
use super::{EditTarget, InsertTarget};
use bytes::Bytes;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_ace::RecoverableNativeEditPlan;
use tablecraft_core::values::Value;

pub(in crate::editing) struct ReviewedState {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: EditTarget,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) replacement: Value,
    pub(in crate::editing) plan: RecoverableNativeEditPlan,
    pub(in crate::editing) valid: Arc<AtomicBool>,
}

pub(crate) struct DocumentEdits {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) valid: Arc<AtomicBool>,
    pub(in crate::editing) draft: Option<(u64, EditTarget)>,
    pub(in crate::editing) insert_draft: Option<(u64, InsertTarget)>,
}

impl DocumentEdits {
    pub(crate) fn new(path: PathBuf, original: Bytes) -> Self {
        Self {
            path,
            original,
            valid: Arc::new(AtomicBool::new(true)),
            draft: None,
            insert_draft: None,
        }
    }
}

impl Drop for DocumentEdits {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
    }
}
