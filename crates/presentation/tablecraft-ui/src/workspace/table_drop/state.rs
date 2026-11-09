use super::{
    TableDropRequest,
    model::{Observation, Phase, SHARED_REQUEST_CONTROL_BYTES, Session},
};
use crate::workspace::{Action, ObjectKind, ObjectTarget, State, field_name::bounded_name};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(0);

impl State {
    /// Closed local user objects only; native profile and dependency admission stay in runtime.
    pub fn can_begin_table_drop(&self, target: &ObjectTarget) -> bool {
        self.table_drop_available
            && self.table_drop.session.is_none()
            && !self.native_mutation_blocked()
            && self.object_switch_ready()
            && !self.has_open_native_objects()
            && self.table_drop_binding(target)
    }

    /// Capture the selected closed target before opening its deletion confirmation.
    pub fn begin_table_drop(&mut self, target: ObjectTarget) -> bool {
        self.synchronize_objects();
        if !self.can_begin_table_drop(&target) {
            return false;
        }
        let ObjectKind::Table { name } = &target.kind else {
            return false;
        };
        let additional = size_of::<Session>()
            .checked_add(size_of::<TableDropRequest>())
            .and_then(|bytes| bytes.checked_add(SHARED_REQUEST_CONTROL_BYTES))
            .and_then(|bytes| bytes.checked_add(name.capacity()));
        if !self.admit_object_bytes(additional) {
            return false;
        }
        let Ok(previous) =
            NEXT_REQUEST.try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return false;
        };
        self.table_drop.session = Some(Box::new(Session {
            request: Arc::new(TableDropRequest {
                id: previous + 1,
                target,
            }),
            phase: Phase::Confirming,
            observation: Observation::Unrequested,
            focus_yes: true,
            cancel_review_requested: false,
            status: None,
            cleanup: None,
            visible: true,
            uncertain: false,
        }));
        true
    }

    pub fn table_drop_open(&self) -> bool {
        self.table_drop
            .session
            .as_ref()
            .is_some_and(|session| session.visible)
    }

    /// Borrow intent for host validation; this supplies no native deletion authority.
    pub fn table_drop_request(&self) -> Option<&TableDropRequest> {
        Some(self.table_drop.session.as_ref()?.request.as_ref())
    }

    pub(super) fn table_drop_binding(&self, target: &ObjectTarget) -> bool {
        let ObjectKind::Table { name } = &target.kind else {
            return false;
        };
        !self.native_source_stale()
            && bounded_name(name)
            && !self.has_open_native_objects()
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == target.document_revision
                    && database
                        .tables
                        .iter()
                        .filter(|table| table.name == *name && table.local && !table.system)
                        .count()
                        == 1
            })
    }

    pub(in crate::workspace) fn collect_table_drop_inspection(
        &mut self,
        actions: &mut Vec<Action>,
    ) {
        let Some(session) = self.table_drop.session.as_mut() else {
            return;
        };
        if session.phase != Phase::Confirming
            || !matches!(session.observation, Observation::Unrequested)
        {
            return;
        }
        session.observation = Observation::Reserved;
        // NO_CLONE: The UI retains exact confirmation intent while the host owns the same
        // immutable request across its asynchronous readonly inspection. Arc shares the
        // bounded target; borrowing cannot outlive the frame, and Rc cannot cross hosts.
        let request = Arc::clone(&session.request);
        actions.push(Action::InspectTableDrop { request });
    }

    pub fn begin_table_drop_inspection(
        &mut self,
        request: &TableDropRequest,
        read_id: u64,
    ) -> bool {
        if read_id == 0
            || !self.table_drop.matches(request)
            || !self.table_drop_binding(&request.target)
        {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.phase != Phase::Confirming
            || !matches!(session.observation, Observation::Reserved)
        {
            return false;
        }
        session.observation = Observation::Pending(read_id);
        true
    }

    pub fn table_drop_inspection_matches(&self, request: &TableDropRequest, read_id: u64) -> bool {
        self.table_drop.matches(request)
            && self.table_drop_binding(&request.target)
            && self.table_drop.session.as_ref().is_some_and(|session| {
                session.phase == Phase::Confirming
                    && matches!(session.observation, Observation::Pending(id) if id == read_id)
            })
    }

    /// Publish actual native metadata only after matching the complete readonly owner.
    pub fn set_table_drop_source(
        &mut self,
        request: &TableDropRequest,
        read_id: u64,
        declared_rows: Option<u32>,
    ) -> bool {
        if !self.table_drop_inspection_matches(request, read_id) {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        session.observation = declared_rows.map_or(Observation::Unsupported, |declared_rows| {
            Observation::Eligible { declared_rows }
        });
        session.focus_yes = declared_rows.is_some();
        true
    }

    /// Failed queue admission has no runtime read ID; do not invent a sentinel ID.
    pub fn reject_table_drop_inspection(
        &mut self,
        request: &TableDropRequest,
        message: String,
    ) -> bool {
        if !self.table_drop.matches(request)
            || !self.table_drop.session.as_ref().is_some_and(|session| {
                session.phase == Phase::Confirming
                    && matches!(session.observation, Observation::Reserved)
            })
        {
            return false;
        }
        self.record_table_drop_diagnosis(message);
        true
    }

    pub fn fail_table_drop_inspection(
        &mut self,
        request: &TableDropRequest,
        read_id: u64,
        message: String,
    ) -> bool {
        if !self.table_drop_inspection_matches(request, read_id) {
            return false;
        }
        self.record_table_drop_diagnosis(message);
        true
    }

    pub fn interrupt_table_drop_inspection(&mut self, read_id: u64) -> bool {
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.phase != Phase::Confirming
            || !matches!(session.observation, Observation::Pending(id) if id == read_id)
        {
            return false;
        }
        session.observation = Observation::Interrupted;
        true
    }

    pub fn table_drop_review_matches(&self, request: &TableDropRequest, read_id: u64) -> bool {
        self.table_drop.matches(request)
            && self.table_drop_binding(&request.target)
            && self.table_drop.session.as_ref().is_some_and(|session| {
                session.phase
                    == (Phase::Reviewing {
                        read_id: Some(read_id),
                    })
            })
    }

    pub fn begin_table_drop_review(&mut self, request: &TableDropRequest, read_id: u64) -> bool {
        if read_id == 0
            || !self.table_drop.matches(request)
            || !self.table_drop_binding(&request.target)
        {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.phase != (Phase::Reviewing { read_id: None }) {
            return false;
        }
        session.phase = Phase::Reviewing {
            read_id: Some(read_id),
        };
        true
    }

    pub fn reject_table_drop_review(
        &mut self,
        request: &TableDropRequest,
        message: String,
    ) -> bool {
        if !self.table_drop.matches(request)
            || !self
                .table_drop
                .session
                .as_ref()
                .is_some_and(|session| session.phase == (Phase::Reviewing { read_id: None }))
        {
            return false;
        }
        self.record_table_drop_diagnosis(message);
        true
    }

    pub fn fail_table_drop_review(
        &mut self,
        request: &TableDropRequest,
        read_id: u64,
        message: String,
    ) -> bool {
        if !self.table_drop_review_matches(request, read_id) {
            return false;
        }
        self.record_table_drop_diagnosis(message);
        true
    }

    pub fn cancel_table_drop_review(&mut self, request: &TableDropRequest, read_id: u64) -> bool {
        if !self.table_drop_review_matches(request, read_id) {
            return false;
        }
        self.table_drop.session = None;
        true
    }

    fn record_table_drop_diagnosis(&mut self, message: String) {
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let Some(session) = self.table_drop.session.as_mut() else {
            return;
        };
        session.phase = Phase::Confirming;
        session.observation = Observation::Failed(if admitted { message } else { String::new() });
        session.cancel_review_requested = false;
    }
}
