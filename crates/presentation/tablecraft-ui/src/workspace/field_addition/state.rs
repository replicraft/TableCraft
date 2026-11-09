//! State admission and exact snapshot boundaries; native checks remain runtime-owned.
use super::{FieldAdditionInspectionError, FieldAdditionRequest, Phase, copy_tab, table_name};
use crate::workspace::{DocumentView, ObjectKind, ObjectTab, State, objects::MAX_RETAINED_BYTES};
use tablecraft_builders::table::{FieldAdditionDeclaration, FieldAdditionKind};

impl State {
    /// Default-false host support must be paired with actual source observation.
    /// Inspection itself acquires no write capability and creates no dirty draft.
    pub fn begin_field_addition_inspection(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
    ) -> Result<(), FieldAdditionInspectionError> {
        self.prepare_field_addition_inspection(tab)?;
        if self.bind_field_addition_inspection(tab, request_id) {
            Ok(())
        } else {
            Err(FieldAdditionInspectionError::Unavailable)
        }
    }

    /// Reserve the exact cache slot, target spelling and memory BEFORE issuing
    /// even the read-only native task; binding its real ID is nonallocating.
    pub fn prepare_field_addition_inspection(
        &mut self,
        tab: &ObjectTab,
    ) -> Result<(), FieldAdditionInspectionError> {
        self.prune_field_addition_inspections();
        if !self.needs_field_addition_inspection(tab) {
            return Err(FieldAdditionInspectionError::Unavailable);
        }
        let Some(copy) = copy_tab(tab) else {
            return Err(FieldAdditionInspectionError::Allocation);
        };
        let storage = match self.field_addition.reserve_inspection(&copy) {
            Ok(storage) => storage,
            Err(error) => {
                if matches!(
                    error,
                    FieldAdditionInspectionError::Capacity
                        | FieldAdditionInspectionError::Allocation
                ) {
                    self.error = Some(self.localizer.text("objects-memory-limit"));
                }
                return Err(error);
            }
        };
        let additional = storage
            .capacity()
            .checked_mul(size_of::<super::Inspection>())
            .and_then(|bytes| bytes.checked_add(table_name(&copy)?.capacity()));
        if !self.admit_object_bytes(additional) {
            return Err(FieldAdditionInspectionError::Allocation);
        }
        let result = self.field_addition.install_inspection(storage, copy);
        if matches!(
            result,
            Err(FieldAdditionInspectionError::Capacity | FieldAdditionInspectionError::Allocation)
        ) {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        result
    }

    pub fn bind_field_addition_inspection(&mut self, tab: &ObjectTab, request_id: u64) -> bool {
        self.field_addition.bind_inspection(tab, request_id)
    }

    pub fn prune_field_addition_inspections(&mut self) {
        let objects = &self.objects;
        self.field_addition
            .prune_inspections(|tab| objects.contains_tab(tab));
    }

    pub fn needs_field_addition_inspection(&self, tab: &ObjectTab) -> bool {
        self.field_addition_context(tab)
            && !self.field_addition_open()
            && self.field_addition.needs_inspection(tab)
    }

    /// A terminal source refusal or error is retried only by explicit intent.
    pub fn retry_field_addition_inspection(&mut self, tab: &ObjectTab) -> bool {
        self.field_addition_context(tab) && self.field_addition.retry_inspection(tab)
    }

    pub fn field_addition_inspection(
        &self,
        tab: &ObjectTab,
    ) -> Option<(bool, Option<bool>, Option<&str>)> {
        let observation = self
            .field_addition
            .inspections
            .iter()
            .find(|item| item.tab == *tab)?;
        Some((
            observation.pending(),
            observation.available.map(super::FieldAdditionChoices::any),
            observation.error.as_deref(),
        ))
    }

    pub fn fail_field_addition_inspection(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.field_addition_inspection_binding(tab) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.field_addition.inspection_failed(
            tab,
            request_id,
            if admitted { message } else { String::new() },
        )
    }

    /// The host must match its pending inspection command before publishing.
    /// This also checks the current source revision, exact table and object tab.
    pub fn set_field_addition_availability(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        number: bool,
        short_text: bool,
    ) -> bool {
        self.field_addition_inspection_binding(tab)
            && self
                .field_addition
                .observed(tab, request_id, number, short_text)
    }

    pub fn can_name_field_addition(&self, tab: &ObjectTab, kind: FieldAdditionKind) -> bool {
        self.field_addition_context(tab) && self.field_addition.available(tab, kind)
    }

    /// Begin the decorative field header. `name` remains exact user-facing text;
    /// the named runtime review owns validity, duplicates and native declaration.
    pub fn begin_field_addition_name(
        &mut self,
        tab: &ObjectTab,
        name: String,
        declaration: FieldAdditionDeclaration,
    ) -> bool {
        if !self.can_name_field_addition(tab, declaration.kind()) {
            return false;
        }
        let additional = table_name(tab).and_then(|table| table.len().checked_add(name.capacity()));
        if !self.admit_object_bytes(additional) {
            return false;
        }
        self.field_addition.begin(tab, name, declaration)
    }

    pub(in crate::workspace) fn begin_default_field_addition(
        &mut self,
        tab: &ObjectTab,
        kind: FieldAdditionKind,
    ) -> bool {
        if !self.can_name_field_addition(tab, kind) {
            return false;
        }
        let Some(data) = &self.data else {
            return false;
        };
        let Ok(name) = tablecraft_builders::table::next_field_name(
            data.columns.iter().map(|column| column.name.as_str()),
        ) else {
            self.error = Some(self.localizer.text("objects-open-refused"));
            return false;
        };
        self.begin_field_addition_name(tab, name, FieldAdditionDeclaration::ui_default(kind))
    }

    pub(in crate::workspace) fn field_addition_render_context(
        &mut self,
    ) -> Option<(ObjectTab, super::header::Presentation)> {
        if !self.field_addition_available {
            return None;
        }
        let tab = self.active_object()?;
        if !self.field_addition_binding(&tab) {
            return None;
        }
        let draft = self
            .field_addition
            .presented_draft()
            .filter(|draft| draft.tab == tab);
        let name = match draft {
            Some(draft) => Some((super::copy_text(&draft.column)?, draft.revision)),
            None => None,
        };
        let additional = (2 * tablecraft_builders::table::MAX_DRAFT_NAME_BYTES)
            .checked_add(table_name(&tab)?.capacity())?
            .checked_add(name.as_ref().map_or(0, |(name, _)| name.capacity()))?;
        let enabled = if draft.is_some() {
            self.can_edit_field_addition_name(&tab)
        } else {
            self.field_addition_context(&tab)
                && self
                    .field_addition_choices(&tab)
                    .is_some_and(super::FieldAdditionChoices::any)
        };
        let focus = draft.is_some_and(|draft| draft.focus);
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }
        let choices = self.field_addition_choices(&tab).unwrap_or_default();
        Some((
            tab,
            super::header::Presentation {
                enabled,
                choices,
                name,
                focus,
            },
        ))
    }

    pub(in crate::workspace) fn apply_field_addition_header(
        &mut self,
        tab: &ObjectTab,
        event: Option<super::header::Event>,
    ) {
        if let Some(draft) = &mut self.field_addition.draft
            && draft.tab == *tab
        {
            draft.focus = false;
        }
        match event {
            Some(super::header::Event::Choose(kind)) => {
                self.begin_default_field_addition(tab, kind);
            }
            Some(super::header::Event::Name(name)) => {
                self.set_field_addition_name(tab, name);
            }
            Some(super::header::Event::Discard { revision }) => {
                self.discard_field_addition_name(tab, revision);
            }
            Some(super::header::Event::MemoryLimit) => {
                self.error = Some(self.localizer.text("objects-memory-limit"));
            }
            None => {}
        }
    }

    pub fn field_addition_modal_open(&self) -> bool {
        self.field_addition_status.is_some()
            || self.field_addition.draft().is_some_and(|(_, _, _, phase)| {
                matches!(phase, Phase::Reviewing { .. } | Phase::Reviewed { .. })
            })
    }

    pub fn field_addition_open(&self) -> bool {
        self.field_addition.draft().is_some()
    }

    pub fn field_addition_declaration(&self) -> Option<FieldAdditionDeclaration> {
        Some(self.field_addition.presented_draft()?.declaration)
    }

    pub fn field_addition_choices(&self, tab: &ObjectTab) -> Option<super::FieldAdditionChoices> {
        self.field_addition
            .inspections
            .iter()
            .find(|item| item.tab == *tab)?
            .available
    }

    pub fn field_addition_name(&self) -> Option<(&ObjectTab, &str, u64)> {
        let (tab, name, revision, _) = self.field_addition.draft()?;
        Some((tab, name, revision))
    }

    pub fn can_edit_field_addition_name(&self, tab: &ObjectTab) -> bool {
        self.field_addition_context(tab)
            && self
                .field_addition
                .draft()
                .is_some_and(|(owner, _, _, phase)| owner == tab && phase == Phase::Naming)
    }

    pub fn set_field_addition_name(&mut self, tab: &ObjectTab, name: String) -> bool {
        if !self.can_edit_field_addition_name(tab)
            || !self.admit_object_bytes(Some(name.capacity()))
        {
            return false;
        }
        self.field_addition.replace_name(tab, name)
    }

    pub fn can_review_field_addition(&self) -> bool {
        self.active_object().is_some_and(|tab| {
            self.can_edit_field_addition_name(&tab)
                && self
                    .field_addition
                    .draft()
                    .is_some_and(|(_, name, _, _)| !name.is_empty())
        })
    }

    pub fn field_addition_review_request(&self) -> Option<FieldAdditionRequest> {
        let tab = self.active_object()?;
        if !self.can_edit_field_addition_name(&tab) {
            return None;
        }
        let request = self.field_addition.request(&tab)?;
        self.object_bytes()?
            .checked_add(request.retained_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(request)
    }

    pub fn begin_field_addition_review(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
    ) -> bool {
        self.field_addition_context(&request.tab)
            && self.field_addition.begin_review(request, request_id)
    }

    /// Exact retained naming snapshot, independent of its current review/save
    /// phase. Structural schema-publication preflight separately checks catalog
    /// binding; successful publication can then clear this old source owner.
    pub fn field_addition_request_is_current(&self, request: &FieldAdditionRequest) -> bool {
        self.field_addition
            .draft()
            .is_some_and(|(tab, column, revision, _)| {
                self.field_addition
                    .presented_draft()
                    .is_some_and(|draft| draft.declaration == request.declaration)
                    && *tab == request.tab
                    && column == request.column
                    && revision == request.draft_revision
            })
    }

    pub fn field_addition_request_current(
        &self,
        request: &FieldAdditionRequest,
        request_id: u64,
    ) -> bool {
        self.field_addition_binding(&request.tab)
            && self
                .field_addition
                .matches(request, Phase::Reviewing { request_id })
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
    }

    pub fn accept_field_addition_review(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        self.field_addition_status.is_none()
            && self.field_addition_request_current(request, request_id)
            && self.field_addition.reviewed(request, request_id, review_id)
    }

    /// Called synchronously only after an actual SaveWorker accepts. Every owned
    /// naming/snapshot allocation must have been admitted before worker.start.
    pub fn bind_field_addition_save(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
        save_id: u64,
    ) -> bool {
        self.field_addition_binding(&request.tab)
            && self
                .field_addition
                .saving(request, request_id, review_id, save_id)
    }

    pub fn release_field_addition_review(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
    ) -> bool {
        self.field_addition.release_review(request, request_id)
    }

    /// A rejected worker start returns its unconsumed capability. This releases
    /// only the matching reviewed presentation, preserving the exact name.
    pub fn release_field_addition_accepted_review(
        &mut self,
        request: &FieldAdditionRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.field_addition.matches(
            request,
            Phase::Reviewed {
                request_id,
                review_id,
            },
        ) {
            return false;
        }
        self.field_addition.draft.as_mut().unwrap().phase = Phase::Naming;
        true
    }

    pub fn discard_field_addition_name(&mut self, tab: &ObjectTab, revision: u64) -> bool {
        self.can_edit_field_addition_name(tab) && self.field_addition.discard(tab, revision)
    }

    fn field_addition_context(&self, tab: &ObjectTab) -> bool {
        self.field_addition_available
            && !self.native_mutation_blocked()
            && !self.field_addition_close_open()
            && self.object_switch_ready_without_saved_field_name()
            && self.view == DocumentView::Table
            && self.field_addition_binding(tab)
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self.saved_query_sql_save_status().is_none()
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(tab.target.document_revision)
                    && table_name(tab).is_some_and(|name| data.table == *name)
            })
    }

    fn field_addition_inspection_binding(&self, tab: &ObjectTab) -> bool {
        self.database.as_ref().is_some_and(|database| {
            database.document_revision == tab.target.document_revision
                && matches!(&tab.target.kind, ObjectKind::Table { name }
                    if database.tables.iter().any(|table| table.name == *name && table.local && !table.system))
        })
    }

    pub(super) fn field_addition_binding(&self, tab: &ObjectTab) -> bool {
        self.active_object().as_ref() == Some(tab)
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == tab.target.document_revision
                    && matches!(&tab.target.kind, ObjectKind::Table { name }
                        if database.tables.iter().any(|table| table.name == *name && table.local && !table.system))
            })
    }
}
