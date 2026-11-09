//! State admission and exact snapshot boundaries; native checks remain runtime-owned.
use super::{NumberColumnInspectionError, NumberColumnRequest, Phase, copy_tab, table_name};
use crate::workspace::{DocumentView, ObjectKind, ObjectTab, State, objects::MAX_RETAINED_BYTES};

impl State {
    /// Default-false host support must be paired with actual source observation.
    /// Inspection itself acquires no write capability and creates no dirty draft.
    pub fn begin_number_column_inspection(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
    ) -> Result<(), NumberColumnInspectionError> {
        self.prepare_number_column_inspection(tab)?;
        if self.bind_number_column_inspection(tab, request_id) {
            Ok(())
        } else {
            Err(NumberColumnInspectionError::Unavailable)
        }
    }

    /// Reserve the exact cache slot, target spelling and memory BEFORE issuing
    /// even the read-only native task; binding its real ID is nonallocating.
    pub fn prepare_number_column_inspection(
        &mut self,
        tab: &ObjectTab,
    ) -> Result<(), NumberColumnInspectionError> {
        self.prune_number_column_inspections();
        if !self.needs_number_column_inspection(tab) {
            return Err(NumberColumnInspectionError::Unavailable);
        }
        let Some(copy) = copy_tab(tab) else {
            return Err(NumberColumnInspectionError::Allocation);
        };
        let storage = match self.number_column.reserve_inspection(&copy) {
            Ok(storage) => storage,
            Err(error) => {
                if matches!(
                    error,
                    NumberColumnInspectionError::Capacity | NumberColumnInspectionError::Allocation
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
            return Err(NumberColumnInspectionError::Allocation);
        }
        let result = self.number_column.install_inspection(storage, copy);
        if matches!(
            result,
            Err(NumberColumnInspectionError::Capacity | NumberColumnInspectionError::Allocation)
        ) {
            self.error = Some(self.localizer.text("objects-memory-limit"));
        }
        result
    }

    pub fn bind_number_column_inspection(&mut self, tab: &ObjectTab, request_id: u64) -> bool {
        self.number_column.bind_inspection(tab, request_id)
    }

    pub fn prune_number_column_inspections(&mut self) {
        let objects = &self.objects;
        self.number_column
            .prune_inspections(|tab| objects.contains_tab(tab));
    }

    pub fn needs_number_column_inspection(&self, tab: &ObjectTab) -> bool {
        self.number_column_context(tab)
            && !self.number_column_open()
            && self.number_column.needs_inspection(tab)
    }

    /// A terminal source refusal or error is retried only by explicit intent.
    pub fn retry_number_column_inspection(&mut self, tab: &ObjectTab) -> bool {
        self.number_column_context(tab) && self.number_column.retry_inspection(tab)
    }

    pub fn number_column_inspection(
        &self,
        tab: &ObjectTab,
    ) -> Option<(bool, Option<bool>, Option<&str>)> {
        let observation = self
            .number_column
            .inspections
            .iter()
            .find(|item| item.tab == *tab)?;
        Some((
            observation.pending(),
            observation.available,
            observation.error.as_deref(),
        ))
    }

    pub fn fail_number_column_inspection(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        message: String,
    ) -> bool {
        if !self.number_column_inspection_binding(tab) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.number_column.inspection_failed(
            tab,
            request_id,
            if admitted { message } else { String::new() },
        )
    }

    /// The host must match its pending inspection command before publishing.
    /// This also checks the current source revision, exact table and object tab.
    pub fn set_number_column_availability(
        &mut self,
        tab: &ObjectTab,
        request_id: u64,
        available: bool,
    ) -> bool {
        self.number_column_inspection_binding(tab)
            && self.number_column.observed(tab, request_id, available)
    }

    pub fn can_name_number_column(&self, tab: &ObjectTab) -> bool {
        self.number_column_context(tab) && self.number_column.available(tab)
    }

    /// Begin the decorative field header. `name` remains exact user-facing text;
    /// the named runtime review owns validity, duplicates and native declaration.
    pub fn begin_number_column_name(&mut self, tab: &ObjectTab, name: String) -> bool {
        if !self.can_name_number_column(tab) {
            return false;
        }
        let additional = table_name(tab).and_then(|table| table.len().checked_add(name.capacity()));
        if !self.admit_object_bytes(additional) {
            return false;
        }
        self.number_column.begin(tab, name)
    }

    pub(in crate::workspace) fn begin_default_number_column(&mut self, tab: &ObjectTab) -> bool {
        if !self.can_name_number_column(tab) {
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
        self.begin_number_column_name(tab, name)
    }

    pub(in crate::workspace) fn number_column_render_context(
        &mut self,
    ) -> Option<(ObjectTab, super::header::Presentation)> {
        if !self.number_column_available {
            return None;
        }
        let tab = self.active_object()?;
        if !self.number_column_binding(&tab) {
            return None;
        }
        let draft = self
            .number_column
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
            self.can_edit_number_column_name(&tab)
        } else {
            self.can_name_number_column(&tab)
        };
        let focus = draft.is_some_and(|draft| draft.focus);
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }
        Some((
            tab,
            super::header::Presentation {
                enabled,
                name,
                focus,
            },
        ))
    }

    pub(in crate::workspace) fn apply_number_column_header(
        &mut self,
        tab: &ObjectTab,
        event: Option<super::header::Event>,
    ) {
        if let Some(draft) = &mut self.number_column.draft
            && draft.tab == *tab
        {
            draft.focus = false;
        }
        match event {
            Some(super::header::Event::Number) => {
                self.begin_default_number_column(tab);
            }
            Some(super::header::Event::Name(name)) => {
                self.set_number_column_name(tab, name);
            }
            Some(super::header::Event::Discard { revision }) => {
                self.discard_number_column_name(tab, revision);
            }
            Some(super::header::Event::MemoryLimit) => {
                self.error = Some(self.localizer.text("objects-memory-limit"));
            }
            None => {}
        }
    }

    pub fn number_column_modal_open(&self) -> bool {
        self.number_column_status.is_some()
            || self.number_column.draft().is_some_and(|(_, _, _, phase)| {
                matches!(phase, Phase::Reviewing { .. } | Phase::Reviewed { .. })
            })
    }

    pub fn number_column_open(&self) -> bool {
        self.number_column.draft().is_some()
    }

    pub fn number_column_name(&self) -> Option<(&ObjectTab, &str, u64)> {
        let (tab, name, revision, _) = self.number_column.draft()?;
        Some((tab, name, revision))
    }

    pub fn can_edit_number_column_name(&self, tab: &ObjectTab) -> bool {
        self.number_column_context(tab)
            && self
                .number_column
                .draft()
                .is_some_and(|(owner, _, _, phase)| owner == tab && phase == Phase::Naming)
    }

    pub fn set_number_column_name(&mut self, tab: &ObjectTab, name: String) -> bool {
        if !self.can_edit_number_column_name(tab) || !self.admit_object_bytes(Some(name.capacity()))
        {
            return false;
        }
        self.number_column.replace_name(tab, name)
    }

    pub fn can_review_number_column(&self) -> bool {
        self.active_object().is_some_and(|tab| {
            self.can_edit_number_column_name(&tab)
                && self
                    .number_column
                    .draft()
                    .is_some_and(|(_, name, _, _)| !name.is_empty())
        })
    }

    pub fn number_column_review_request(&self) -> Option<NumberColumnRequest> {
        let tab = self.active_object()?;
        if !self.can_edit_number_column_name(&tab) {
            return None;
        }
        let request = self.number_column.request(&tab)?;
        self.object_bytes()?
            .checked_add(request.retained_bytes()?)
            .filter(|bytes| *bytes <= MAX_RETAINED_BYTES)?;
        Some(request)
    }

    pub fn begin_number_column_review(
        &mut self,
        request: &NumberColumnRequest,
        request_id: u64,
    ) -> bool {
        self.number_column_context(&request.tab)
            && self.number_column.begin_review(request, request_id)
    }

    /// Exact retained naming snapshot, independent of its current review/save
    /// phase. Structural schema-publication preflight separately checks catalog
    /// binding; successful publication can then clear this old source owner.
    pub fn number_column_request_is_current(&self, request: &NumberColumnRequest) -> bool {
        self.number_column
            .draft()
            .is_some_and(|(tab, column, revision, _)| {
                *tab == request.tab
                    && column == request.column
                    && revision == request.draft_revision
            })
    }

    pub fn number_column_request_current(
        &self,
        request: &NumberColumnRequest,
        request_id: u64,
    ) -> bool {
        self.number_column_binding(&request.tab)
            && self
                .number_column
                .matches(request, Phase::Reviewing { request_id })
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
    }

    pub fn accept_number_column_review(
        &mut self,
        request: &NumberColumnRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        self.number_column_status.is_none()
            && self.number_column_request_current(request, request_id)
            && self.number_column.reviewed(request, request_id, review_id)
    }

    /// Called synchronously only after an actual SaveWorker accepts. Every owned
    /// naming/snapshot allocation must have been admitted before worker.start.
    pub fn bind_number_column_save(
        &mut self,
        request: &NumberColumnRequest,
        request_id: u64,
        review_id: u64,
        save_id: u64,
    ) -> bool {
        self.number_column_binding(&request.tab)
            && self
                .number_column
                .saving(request, request_id, review_id, save_id)
    }

    pub fn release_number_column_review(
        &mut self,
        request: &NumberColumnRequest,
        request_id: u64,
    ) -> bool {
        self.number_column.release_review(request, request_id)
    }

    /// A rejected worker start returns its unconsumed capability. This releases
    /// only the matching reviewed presentation, preserving the exact name.
    pub fn release_number_column_accepted_review(
        &mut self,
        request: &NumberColumnRequest,
        request_id: u64,
        review_id: u64,
    ) -> bool {
        if !self.number_column.matches(
            request,
            Phase::Reviewed {
                request_id,
                review_id,
            },
        ) {
            return false;
        }
        self.number_column.draft.as_mut().unwrap().phase = Phase::Naming;
        true
    }

    pub fn discard_number_column_name(&mut self, tab: &ObjectTab, revision: u64) -> bool {
        self.can_edit_number_column_name(tab) && self.number_column.discard(tab, revision)
    }

    fn number_column_context(&self, tab: &ObjectTab) -> bool {
        self.number_column_available
            && !self.number_column_source_stale()
            && !self.number_column_close_open()
            && self.object_switch_ready_without_number_column()
            && self.view == DocumentView::Table
            && self.number_column_binding(tab)
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self.saved_query_sql_save_status().is_none()
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(tab.target.document_revision)
                    && table_name(tab).is_some_and(|name| data.table == *name)
            })
    }

    fn number_column_inspection_binding(&self, tab: &ObjectTab) -> bool {
        self.database.as_ref().is_some_and(|database| {
            database.document_revision == tab.target.document_revision
                && matches!(&tab.target.kind, ObjectKind::Table { name }
                    if database.tables.iter().any(|table| table.name == *name && table.local && !table.system))
        })
    }

    pub(super) fn number_column_binding(&self, tab: &ObjectTab) -> bool {
        self.active_object().as_ref() == Some(tab)
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == tab.target.document_revision
                    && matches!(&tab.target.kind, ObjectKind::Table { name }
                        if database.tables.iter().any(|table| table.name == *name && table.local && !table.system))
            })
    }
}
