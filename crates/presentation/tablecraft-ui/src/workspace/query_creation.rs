//! Exact presentation association for authored QueryDef creation; no native authority.
mod dialog;
mod request;
mod save;
#[cfg(test)]
mod tests;

pub use save::{QueryCreationSaveCommand, QueryCreationSaveIdentity, QueryCreationSaveOperation};

use super::{ObjectTab, SavedQueryTarget};
use crate::workspace::objects::memory::Counter;
use std::sync::{Arc, atomic::AtomicUsize};

pub(super) const SHARED_TEXT_CONTROL_BYTES: usize =
    size_of::<String>() + size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<String>();
pub(super) const SHARED_REQUEST_CONTROL_BYTES: usize =
    size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<QueryCreationRequest>();

/// The submitted editor snapshot. Native source and SQL qualification remain host-owned.
#[derive(Debug, PartialEq, Eq)]
pub struct QueryCreationRequest {
    pub tab: ObjectTab,
    pub document_revision: u64,
    pub text_revision: u64,
    pub design_revision: Option<u64>,
    pub source: Option<SavedQueryTarget>,
    pub sql: Arc<String>,
    pub destination: String,
}

#[derive(Default)]
pub(super) struct Model {
    name: Option<request::Name>,
    pending: Option<Pending>,
    save: Option<save::SaveStatus>,
}

struct Pending {
    request_id: u64,
    request: Arc<QueryCreationRequest>,
    review_id: Option<u64>,
}

impl Model {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(name) = &self.name {
            count.string(&name.text);
            if let super::ObjectKind::SavedQuery { name, .. } = &name.tab.target.kind {
                count.string(name);
            }
        }
        if let Some(pending) = &self.pending {
            count.add(pending.request.retained_bytes()?);
        }
        if let Some(status) = &self.save {
            let already_counted = self
                .pending
                .as_ref()
                .is_some_and(|pending| Arc::ptr_eq(&pending.request, &status.request));
            if !already_counted {
                count.add(status.request.retained_bytes()?);
            }
            if let Some(outcome) = &status.outcome {
                count.add(super::saved_query_save::outcome_bytes(outcome));
            }
            count.optional(&status.reopen_error);
        }
        count.finish()
    }
}

impl QueryCreationRequest {
    fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.add(SHARED_REQUEST_CONTROL_BYTES);
        count.string(self.sql.as_ref());
        count.add(SHARED_TEXT_CONTROL_BYTES);
        count.string(&self.destination);
        if let super::ObjectKind::SavedQuery { name, .. } = &self.tab.target.kind {
            count.string(name);
        }
        if let Some(source) = &self.source {
            count.string(&source.name);
        }
        count.finish()
    }
}
