use super::{TableDropRequest, TableDropSaveIdentity};
use crate::workspace::{
    ObjectKind, SaveCleanupNotice, objects::memory::Counter, schema_save::Status,
};
use std::sync::{Arc, atomic::AtomicUsize};

// Conservatively reserve both reference counters and a complete alignment unit.
// This allowance is not a public promise about the standard library's Arc ABI.
pub(super) const SHARED_REQUEST_CONTROL_BYTES: usize =
    size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<TableDropRequest>();

#[derive(Default)]
pub(in crate::workspace) struct Model {
    pub(super) session: Option<Box<Session>>,
}

pub(super) struct Session {
    pub request: Arc<TableDropRequest>,
    pub phase: Phase,
    pub observation: Observation,
    pub focus_yes: bool,
    pub cancel_review_requested: bool,
    pub status: Option<Status<TableDropSaveIdentity>>,
    pub cleanup: Option<SaveCleanupNotice>,
    pub visible: bool,
    pub uncertain: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Confirming,
    Reviewing { read_id: Option<u64> },
    Owned(TableDropSaveIdentity),
}

pub(super) enum Observation {
    Unrequested,
    Reserved,
    Pending(u64),
    Eligible { declared_rows: u32 },
    Unsupported,
    Interrupted,
    Failed(String),
}

impl Model {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(session) = &self.session {
            count.add(size_of::<Session>());
            count.add(size_of::<TableDropRequest>());
            count.add(SHARED_REQUEST_CONTROL_BYTES);
            if let ObjectKind::Table { name } = &session.request.target.kind {
                count.string(name);
            }
            if let Observation::Failed(message) = &session.observation {
                count.string(message);
            }
            if let Some(status) = &session.status {
                count.add(status.heap_bytes()?);
            }
        }
        count.finish()
    }

    pub(super) fn matches(&self, request: &TableDropRequest) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.request.as_ref() == request)
    }
}
