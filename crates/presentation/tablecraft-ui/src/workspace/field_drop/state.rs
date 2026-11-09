use super::{
    FieldDropConfirmationDecision as Decision, FieldDropConfirmationResult, FieldDropRequest,
    FieldDropTarget,
    model::{Observation, Phase, RequestReservation},
};
use crate::workspace::{DocumentView, ObjectKind, State, objects::MAX_RETAINED_BYTES};

impl State {
    pub fn can_begin_field_drop(&self, target: &FieldDropTarget) -> bool {
        self.field_drop_available
            && self.field_drop.draft.is_none()
            && self.field_drop.warning.is_none()
            && target.bounded()
            && self.field_drop_context(target)
    }

    pub fn begin_field_drop(&mut self, target: FieldDropTarget) -> bool {
        if !self.can_begin_field_drop(&target) || !self.admit_object_bytes(target.heap_bytes()) {
            return false;
        }
        self.field_drop.begin(target)
    }

    pub fn field_drop_target(&self) -> Option<&FieldDropTarget> {
        Some(&self.field_drop.presented()?.target)
    }

    pub fn field_drop_open(&self) -> bool {
        self.field_drop.presented().is_some()
    }

    pub fn field_drop_dirty(&self) -> bool {
        self.field_drop.dirty()
    }

    /// A pending mark does not remove fields from stored schema/data presentation.
    pub fn pending_field_drop(&self) -> Option<(&FieldDropTarget, u64)> {
        let draft = self.field_drop.presented()?;
        (draft.phase != Phase::Confirming).then_some((&draft.target, draft.revision))
    }

    pub fn field_drop_confirmation_open(&self) -> bool {
        self.field_drop
            .draft
            .as_ref()
            .is_some_and(|draft| draft.phase == Phase::Confirming)
    }

    pub fn can_confirm_field_drop(&self, target: &FieldDropTarget, revision: u64) -> bool {
        self.field_drop.draft.as_ref().is_some_and(|draft| {
            draft.target == *target
                && draft.revision == revision
                && draft.phase == Phase::Confirming
                && matches!(draft.observation, Observation::Eligible)
                && self.field_drop_context(target)
        })
    }

    pub fn choose_field_drop_confirmation(
        &mut self,
        target: &FieldDropTarget,
        revision: u64,
        decision: Decision,
    ) -> Option<FieldDropConfirmationResult> {
        let draft = self.field_drop.draft.as_ref()?;
        if draft.target != *target || draft.revision != revision || draft.phase != Phase::Confirming
        {
            return None;
        }
        if decision == Decision::Yes && !self.can_confirm_field_drop(target, revision) {
            return None;
        }
        self.field_drop.choose_confirmation(decision)
    }

    /// Dismiss only the current unconfirmed owner, without allocation or write.
    /// Hosts revoke only the returned actual readonly publication before continuing.
    pub fn dismiss_field_drop_confirmation(&mut self) -> Option<FieldDropConfirmationResult> {
        if self.field_drop.status.is_some() || self.field_drop.close.is_some() {
            return None;
        }
        self.field_drop.choose_confirmation(Decision::No)
    }

    pub fn field_drop_inspection_target(&self) -> Option<FieldDropTarget> {
        let draft = self.field_drop.draft.as_ref()?;
        if draft.phase != Phase::Confirming
            || !matches!(draft.observation, Observation::Pending { request_id: None })
            || !self.field_drop_context(&draft.target)
        {
            return None;
        }
        let target = draft.target.copy()?;
        self.object_bytes()?
            .checked_add(size_of::<FieldDropTarget>())?
            .checked_add(target.heap_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(target)
    }

    pub fn pending_field_drop_inspection_id(&self) -> Option<u64> {
        match self.field_drop.draft.as_ref()?.observation {
            Observation::Pending { request_id } => request_id,
            _ => None,
        }
    }

    pub fn field_drop_inspection_matches(&self, target: &FieldDropTarget, request_id: u64) -> bool {
        self.field_drop_owner_bound(target)
            && self.field_drop.inspection_matches(target, request_id)
    }

    pub fn set_field_drop_source(
        &mut self,
        target: &FieldDropTarget,
        request_id: u64,
        source_eligible: bool,
    ) -> bool {
        if !self.field_drop_inspection_matches(target, request_id) {
            return false;
        }
        self.field_drop.draft.as_mut().unwrap().observation = if source_eligible {
            Observation::Eligible
        } else {
            Observation::Unsupported
        };
        true
    }

    /// Match borrowed runtime facts without allocating another target.
    pub fn set_field_drop_inspection_by_id(
        &mut self,
        request_id: u64,
        document_revision: u64,
        table: &str,
        column: &str,
        native_number: usize,
        source_eligible: bool,
    ) -> bool {
        let Some(draft) = self.field_drop.draft.as_ref() else {
            return false;
        };
        if draft.target.tab.target.document_revision != document_revision
            || draft.target.column != column
            || draft.target.native_number != native_number
            || !matches!(&draft.target.tab.target.kind, ObjectKind::Table { name } if name == table)
            || !self.field_drop_owner_bound(&draft.target)
            || !self
                .field_drop
                .inspection_matches(&draft.target, request_id)
        {
            return false;
        }
        self.field_drop.draft.as_mut().unwrap().observation = if source_eligible {
            Observation::Eligible
        } else {
            Observation::Unsupported
        };
        true
    }

    pub fn fail_field_drop_inspection(
        &mut self,
        target: &FieldDropTarget,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.field_drop_inspection_matches(target, request_id) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_drop.draft.as_mut().unwrap().observation = Observation::Failed {
            message: if admitted { message } else { String::new() },
        };
        true
    }

    pub fn interrupt_field_drop_inspection(
        &mut self,
        target: &FieldDropTarget,
        request_id: u64,
    ) -> bool {
        if !self.field_drop_inspection_matches(target, request_id) {
            return false;
        }
        self.field_drop.draft.as_mut().unwrap().observation = Observation::Interrupted;
        true
    }

    /// Retire the exact recorded read without copying its retained target.
    pub fn fail_field_drop_inspection_by_id(&mut self, request_id: u64, message: String) -> bool {
        let Some(draft) = self.field_drop.draft.as_ref() else {
            return false;
        };
        if !self
            .field_drop
            .inspection_matches(&draft.target, request_id)
        {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_drop.draft.as_mut().unwrap().observation = Observation::Failed {
            message: if admitted { message } else { String::new() },
        };

        true
    }

    /// Publication revocation must release its pending owner despite focus changes.
    pub fn interrupt_field_drop_inspection_id(&mut self, request_id: u64) -> bool {
        let Some(draft) = self.field_drop.draft.as_ref() else {
            return false;
        };
        if !self
            .field_drop
            .inspection_matches(&draft.target, request_id)
        {
            return false;
        }

        self.field_drop.draft.as_mut().unwrap().observation = Observation::Interrupted;

        true
    }

    pub fn retry_field_drop_inspection(&mut self, target: &FieldDropTarget) -> bool {
        if !self.field_drop_context(target) {
            return false;
        }
        let Some(draft) = self.field_drop.draft.as_mut() else {
            return false;
        };
        if draft.target != *target
            || draft.phase != Phase::Confirming
            || matches!(draft.observation, Observation::Pending { .. })
        {
            return false;
        }
        draft.observation = Observation::Pending { request_id: None };
        true
    }

    pub fn can_review_field_drop(&self) -> bool {
        self.field_drop.close.is_none()
            && self.field_drop.status.is_none()
            && self.field_drop.request_memory.is_none()
            && self.field_drop.draft.as_ref().is_some_and(|draft| {
                draft.phase == Phase::PendingDeletion
                    && matches!(draft.observation, Observation::Eligible)
                    && self.field_drop_context(&draft.target)
            })
    }

    pub fn field_drop_review_request(&self) -> Option<FieldDropRequest> {
        if !self.can_review_field_drop() {
            return None;
        }
        let draft = self.field_drop.draft.as_ref()?;
        let request = FieldDropRequest {
            target: draft.target.copy()?,
            draft_revision: draft.revision,
        };
        self.object_bytes()?
            .checked_add(request.retained_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(request)
    }

    pub fn begin_field_drop_review(&mut self, request: &FieldDropRequest, request_id: u64) -> bool {
        if !self.can_review_field_drop()
            || !self.field_drop.matches(request, Phase::PendingDeletion)
        {
            return false;
        }
        let Some(bytes) = request.retained_bytes() else {
            return false;
        };
        if !self.admit_object_bytes(Some(bytes)) {
            return false;
        }
        self.field_drop.request_memory = Some(RequestReservation { request_id, bytes });
        self.field_drop.draft.as_mut().unwrap().phase = Phase::Reviewing { request_id };
        true
    }

    pub fn field_drop_request_is_current(&self, request: &FieldDropRequest) -> bool {
        self.field_drop.draft.as_ref().is_some_and(|draft| {
            draft.target == request.target && draft.revision == request.draft_revision
        })
    }

    pub fn field_drop_request_current(&self, request: &FieldDropRequest, request_id: u64) -> bool {
        self.field_drop_owner_bound(&request.target)
            && self
                .field_drop
                .matches(request, Phase::Reviewing { request_id })
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && !self.field_addition_open()
            && !self.field_rename_open()
            && !self.database_options_pending()
    }

    pub fn accept_field_drop_review(
        &mut self,
        request: &FieldDropRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if self.field_drop.status.is_some() || !self.field_drop_request_current(request, request_id)
        {
            return false;
        }
        self.field_drop.draft.as_mut().unwrap().phase = Phase::Reviewed {
            request_id,
            review_id,
        };
        true
    }

    /// Release the exact host request after review failure or abandoned save start.
    /// The host drops its snapshot immediately; no new memory admission is needed.
    pub fn release_field_drop_review(
        &mut self,
        request: &FieldDropRequest,
        request_id: u64,
    ) -> bool {
        let reviewing = self
            .field_drop
            .matches(request, Phase::Reviewing { request_id });
        let abandoned_start = self.field_drop.matches(request, Phase::PendingDeletion)
            && self.field_drop.reserves_request(request, request_id);
        if !reviewing && !abandoned_start {
            return false;
        }
        // The host releases this exact request. An abandoned save-start token
        // only rolls back its phase; it must not erase still-live host memory.
        self.field_drop.request_memory = None;
        self.field_drop.draft.as_mut().unwrap().phase = Phase::PendingDeletion;
        true
    }

    /// Release a checked review whose host will not start the save actor.
    /// The host drops its exact request snapshot immediately after this transition.
    pub fn release_field_drop_accepted_review(
        &mut self,
        request: &FieldDropRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.field_drop.matches(
            request,
            Phase::Reviewed {
                request_id,
                review_id,
            },
        ) {
            return false;
        }
        self.field_drop.request_memory = None;
        self.field_drop.draft.as_mut().unwrap().phase = Phase::PendingDeletion;
        true
    }

    pub(super) fn field_drop_context(&self, target: &FieldDropTarget) -> bool {
        self.field_drop.close.is_none() && self.field_drop_context_without_close(target)
    }

    pub(super) fn field_drop_context_without_close(&self, target: &FieldDropTarget) -> bool {
        self.field_drop_available
            && !self.native_mutation_blocked_without_field_drop()
            && self.object_switch_ready_without_saved_field_name()
            && !self.field_addition_open()
            && !self.field_addition_close_open()
            && !self.field_rename_open()
            && !self.field_rename_close_open()
            && self.field_drop_owner_bound(target)
            && self.field_drop_metadata_matches(target)
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
    }

    pub(in crate::workspace) fn field_drop_owner_bound(&self, target: &FieldDropTarget) -> bool {
        self.objects.is_active_tab(&target.tab)
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == target.tab.target.document_revision
                    && matches!(&target.tab.target.kind, ObjectKind::Table { name }
                        if database.tables.iter().filter(|table| table.name == *name && table.local && !table.system).count() == 1)
            })
    }

    fn field_drop_metadata_matches(&self, target: &FieldDropTarget) -> bool {
        let ObjectKind::Table { name } = &target.tab.target.kind else {
            return false;
        };
        match self.view {
            DocumentView::Table => self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(target.tab.target.document_revision)
                    && data.table == *name
                    && data
                        .columns
                        .iter()
                        .filter(|column| {
                            column.name == target.column
                                && column.native_number == Some(target.native_number)
                        })
                        .count()
                        == 1
            }),
            DocumentView::Design => self.schema.as_ref().is_some_and(|schema| {
                schema.table == *name
                    && schema
                        .fields
                        .iter()
                        .filter(|field| {
                            field.name == target.column && field.number == target.native_number
                        })
                        .count()
                        == 1
            }),
            DocumentView::Query => false,
        }
    }
}
