//! Presentation admission. Native eligibility and consuming review stay runtime-owned.
use super::{
    FieldRenameRequest, FieldRenameTarget, copy_text,
    model::{Observation, Phase},
};
use crate::workspace::{DocumentView, ObjectKind, State, objects::MAX_RETAINED_BYTES};

impl State {
    /// Read-only source inspection begins from exact current native metadata.
    /// An editor never invents a native number from a rendered column position.
    pub fn can_begin_field_rename(&self, target: &FieldRenameTarget) -> bool {
        self.field_rename_available
            && self.field_rename.draft.is_none()
            && self.field_rename.warning.is_none()
            && self.field_rename_context(target)
            && self.field_rename_metadata_matches(target)
    }

    pub fn begin_field_rename(&mut self, target: FieldRenameTarget) -> bool {
        if !self.can_begin_field_rename(&target) {
            return false;
        }

        let Some(proposed) = copy_text(&target.column) else {
            self.field_rename_memory_error();
            return false;
        };
        let additional = target
            .heap_bytes()
            .and_then(|bytes| bytes.checked_add(proposed.capacity()));
        if !self.admit_object_bytes(additional) {
            return false;
        }

        self.field_rename.begin(target, proposed)
    }

    /// Retain a bounded exact target before starting even the readonly worker.
    /// Binding the real worker ID afterwards performs no allocation.
    pub fn field_rename_inspection_target(&self) -> Option<FieldRenameTarget> {
        let draft = self.field_rename.draft.as_ref()?;
        if !matches!(draft.observation, Observation::Pending { request_id: None }) {
            return None;
        }

        let target = draft.target.copy()?;
        self.object_bytes()?
            .checked_add(size_of::<FieldRenameTarget>())?
            .checked_add(target.heap_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(target)
    }

    pub fn field_rename_target(&self) -> Option<&FieldRenameTarget> {
        Some(&self.field_rename.presented()?.target)
    }

    pub fn pending_field_rename_inspection_id(&self) -> Option<u64> {
        match self.field_rename.draft.as_ref()?.observation {
            Observation::Pending { request_id } => request_id,
            _ => None,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Check borrowed source identity without allocating a second target."
    )]
    pub fn set_field_rename_inspection_by_id(
        &mut self,
        request_id: u64,
        document_revision: u64,
        table: &str,
        column: &str,
        native_number: usize,
        source_eligible: bool,
        same_width_ascii_utf16_units: Option<usize>,
    ) -> bool {
        let Some(draft) = self.field_rename.draft.as_ref() else {
            return false;
        };
        if draft.target.tab.target.document_revision != document_revision
            || draft.target.column != column
            || draft.target.native_number != native_number
            || !matches!(&draft.target.tab.target.kind, ObjectKind::Table { name } if name == table)
        {
            return false;
        }

        self.field_rename
            .observed_by_id(request_id, source_eligible, same_width_ascii_utf16_units)
    }

    pub fn fail_field_rename_inspection_by_id(&mut self, request_id: u64, message: String) -> bool {
        if self.pending_field_rename_inspection_id() != Some(request_id) {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let Some(draft) = self.field_rename.pending_mut(request_id) else {
            return false;
        };
        draft.observation = Observation::Failed {
            message: if admitted { message } else { String::new() },
        };

        true
    }

    pub fn interrupt_field_rename_inspection_id(&mut self, request_id: u64) -> bool {
        let Some(draft) = self.field_rename.pending_mut(request_id) else {
            return false;
        };
        draft.observation = Observation::Interrupted;

        true
    }

    pub fn field_rename_inspection_matches(
        &self,
        target: &FieldRenameTarget,
        request_id: u64,
    ) -> bool {
        self.field_rename.inspection_matches(target, request_id)
    }

    /// The host must first match its actual command/reply identity. Busy,
    /// focus and modal changes must not strand this recorded terminal owner.
    pub fn set_field_rename_source(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
        source_eligible: bool,
        same_width_ascii_utf16_units: Option<usize>,
    ) -> bool {
        self.field_rename.observed(
            target,
            request_id,
            source_eligible,
            same_width_ascii_utf16_units,
        )
    }

    pub fn fail_field_rename_inspection(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.field_rename.inspection_matches(target, request_id) {
            return false;
        }

        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_rename.inspection_failed(
            target,
            request_id,
            if admitted { message } else { String::new() },
        )
    }

    pub fn interrupt_field_rename_inspection(
        &mut self,
        target: &FieldRenameTarget,
        request_id: u64,
    ) -> bool {
        self.field_rename.inspection_interrupted(target, request_id)
    }

    pub fn retry_field_rename_inspection(&mut self, target: &FieldRenameTarget) -> bool {
        self.field_rename_context(target) && self.field_rename.retry_inspection(target)
    }

    pub fn field_rename_open(&self) -> bool {
        self.field_rename.presented().is_some()
    }

    pub fn field_rename_dirty(&self) -> bool {
        self.field_rename.dirty()
    }

    pub fn field_rename_proposal(&self) -> Option<(&FieldRenameTarget, &str, u64)> {
        let draft = self.field_rename.presented()?;
        Some((&draft.target, &draft.proposed, draft.revision))
    }

    pub fn can_edit_field_rename(&self) -> bool {
        let Some(draft) = &self.field_rename.draft else {
            return false;
        };
        self.field_rename_context(&draft.target)
            && draft.phase == Phase::Naming
            && matches!(draft.observation, Observation::Eligible { .. })
    }

    /// Keep all bounded proposed text. ASCII/equal-width constraints control
    /// review availability, never automatic truncation or spelling conversion.
    pub fn set_field_rename_name(&mut self, target: &FieldRenameTarget, name: String) -> bool {
        if !self.can_edit_field_rename()
            || self
                .field_rename
                .draft
                .as_ref()
                .is_none_or(|draft| draft.target != *target)
        {
            return false;
        }
        if !self.admit_object_bytes(Some(name.capacity())) {
            return false;
        }

        self.field_rename.replace(target, name)
    }

    pub fn can_review_field_rename(&self) -> bool {
        self.can_edit_field_rename() && self.field_rename.profile_fits()
    }

    pub fn field_rename_review_request(&self) -> Option<FieldRenameRequest> {
        if !self.can_review_field_rename() {
            return None;
        }

        let draft = self.field_rename.draft.as_ref()?;
        let request = FieldRenameRequest {
            target: draft.target.copy()?,
            draft_revision: draft.revision,
            new_name: copy_text(&draft.proposed)?,
        };
        self.object_bytes()?
            .checked_add(request.retained_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(request)
    }

    pub fn begin_field_rename_review(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
    ) -> bool {
        self.field_rename_context(&request.target)
            && self.field_rename.begin_review(request, request_id)
    }

    pub fn field_rename_request_current(
        &self,
        request: &FieldRenameRequest,
        request_id: u64,
    ) -> bool {
        self.field_rename_owner_bound(&request.target)
            && self
                .field_rename
                .matches(request, Phase::Reviewing { request_id })
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && !self.field_addition_open()
            && !self.database_options_pending()
    }

    pub fn field_rename_request_is_current(&self, request: &FieldRenameRequest) -> bool {
        self.field_rename.draft.as_ref().is_some_and(|draft| {
            draft.target == request.target
                && draft.proposed == request.new_name
                && draft.revision == request.draft_revision
        })
    }

    pub fn accept_field_rename_review(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        self.field_rename.status.is_none()
            && self.field_rename_request_current(request, request_id)
            && self.field_rename.reviewed(request, request_id, review_id)
    }

    pub fn release_field_rename_review(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
    ) -> bool {
        self.field_rename.release_review(request, request_id)
    }

    pub fn release_field_rename_accepted_review(
        &mut self,
        request: &FieldRenameRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        self.field_rename
            .release_accepted(request, request_id, review_id)
    }

    pub fn discard_field_rename(&mut self, target: &FieldRenameTarget, revision: u64) -> bool {
        self.field_rename.discard(target, revision)
    }

    fn field_rename_context(&self, target: &FieldRenameTarget) -> bool {
        self.field_rename_context_without_close(target) && self.field_rename.close.is_none()
    }

    pub(super) fn field_rename_context_without_close(&self, target: &FieldRenameTarget) -> bool {
        self.field_rename_available
            && !self.native_mutation_blocked_without_field_rename()
            && self.object_switch_ready_without_saved_field_name()
            && !self.field_addition_open()
            && !self.field_addition_close_open()
            && self.field_rename_owner_bound(target)
            && self.field_rename_metadata_matches(target)
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
    }

    pub(in crate::workspace) fn field_rename_owner_bound(
        &self,
        target: &FieldRenameTarget,
    ) -> bool {
        self.objects.is_active_tab(&target.tab)
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == target.tab.target.document_revision
                    && matches!(&target.tab.target.kind, ObjectKind::Table { name }
                        if database.tables.iter().filter(|table| table.name == *name
                            && table.local && !table.system).count() == 1)
            })
    }

    fn field_rename_metadata_matches(&self, target: &FieldRenameTarget) -> bool {
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

    fn field_rename_memory_error(&mut self) {
        self.error = Some(self.localizer.text("objects-memory-limit"));
    }
}
