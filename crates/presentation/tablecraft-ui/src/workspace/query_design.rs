//! Local QBE drafts require explicit source-bound canonicalization before execution.
mod context_menu;
mod dialog;
mod field_inputs;
mod grid;
mod parameters;
mod preferences;
mod rebind_memory;
mod render;
mod retention;
mod ribbon;
mod sources;
mod totals;

pub(in crate::workspace) use preferences::Preferences;

use super::query::provenance::ResultOrigin;
use super::{Action, DocumentView, ObjectKind, ObjectTab, State, click};
use std::sync::Arc;
use tablecraft_builders::query::{
    BuiltQueryDesign, DesignSource, DesignTableMetadata, MAX_DESIGN_COLUMNS, MAX_DESIGN_SOURCES,
    MAX_DESIGN_TEXT_BYTES, QueryDesignDraft,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryDesignTarget {
    pub document_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryDesignRequest {
    pub tab: ObjectTab,
    pub target: QueryDesignTarget,
    pub draft_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryDesignCompletion {
    ShowSql,
    Run,
    Save,
    SaveExisting,
}

#[derive(Default)]
pub(super) struct QueryDesign {
    session: Option<Box<Session>>,
}

#[derive(Default)]
struct Session {
    mode: Mode,
    draft: QueryDesignDraft,
    revision: u64,
    saved_revision: u64,
    field_inputs: field_inputs::FieldInputs,
    result_revision: Option<u64>,
    totals_off_qualification: Option<(u64, bool)>,
    metadata: Vec<DesignTableMetadata>,
    pending: Option<Pending>,
    error: Option<String>,
    add_tables: bool,
    selected_table: Option<String>,
    click_pair: Option<click::Pair>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Sql,
    Design,
    Results,
}

struct Pending {
    id: u64,
    request: QueryDesignRequest,
    publication: u64,
    kind: PendingKind,
}

enum PendingKind {
    Describe {
        table: String,
    },
    Build {
        draft: Arc<QueryDesignDraft>,
        completion: QueryDesignCompletion,
    },
    Import {
        sql: String,
    },
    Run {
        sql: String,
    },
}

impl State {
    pub(in crate::workspace) fn saved_query_data_visible(&self) -> bool {
        self.saved_query.active
            && !self.query_design_visible()
            && !self.query_design_results_visible()
    }

    /// Current authoring text for either the actual scratch or saved Design owner.
    pub fn query_design_sql(&self) -> Option<&str> {
        match self.active_object()?.target.kind {
            ObjectKind::Sql => Some(&self.sql),
            ObjectKind::SavedQuery { .. } => self.saved_query_sql().map(|(_, sql, _, _)| sql),
            _ => None,
        }
    }

    fn replace_query_design_sql(&mut self, sql: String) -> bool {
        if self.saved_query.active {
            if self.query_design_sql() == Some(sql.as_str()) {
                return true;
            }
            if !self.admit_object_bytes(Some(sql.capacity())) {
                return false;
            }
            let Some(draft) = &mut self.saved_query.sql else {
                return false;
            };
            draft.replace_text(sql)
        } else {
            self.replace_query_sql(sql)
        }
    }

    fn set_query_design_data(&mut self, sql: String, mut data: super::Data) -> bool {
        if !self.saved_query.active {
            return self.set_query_data(Some(sql), data);
        }
        if data.source_revision.is_some()
            || data.row_ordinals.is_some()
            || data
                .query_contents
                .as_ref()
                .is_some_and(|bindings| !bindings.valid_for(&data))
        {
            return false;
        }
        let submitted = Some(sql);
        if !self.admit_query_data(&submitted, &data) {
            return false;
        }
        self.prepare_display(&mut data);
        if !self.admit_query_data(&submitted, &data) {
            return false;
        }
        self.bump_focus_publication();
        self.query_selection.reset(&data);
        self.query_data = Some(data);
        self.query_result = submitted.map_or(ResultOrigin::Unknown, ResultOrigin::Sql);
        true
    }

    pub fn query_design_request(&self) -> Option<QueryDesignRequest> {
        let tab = self.active_object()?;
        let admitted = match &tab.target.kind {
            ObjectKind::Sql => !self.saved_query.active,
            ObjectKind::SavedQuery { .. } => {
                self.saved_query.active && self.saved_query_sql().is_some()
            }
            _ => false,
        };
        if !admitted {
            return None;
        }
        let revision = self.database.as_ref()?.document_revision;
        if tab.target.document_revision != revision || self.view != DocumentView::Query {
            return None;
        }
        Some(QueryDesignRequest {
            tab,
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft_revision: self.query_design.session.as_ref().map_or(0, |s| s.revision),
        })
    }

    fn query_design_ready(&self) -> bool {
        !self.schema_after_awaits_reopen()
            && self.query_design_request().is_some()
            && !self.busy
            && !self.backstage_visible()
            && self.edit.is_none()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && !self.field_filter_dialog_open()
            && !self.zoom_dialog_open()
            && !self.parameter_prompt_open()
            && self.contents_request_id().is_none()
            && self.query_design_request_id().is_none()
    }

    pub fn query_design_request_id(&self) -> Option<u64> {
        self.query_design
            .session
            .as_ref()?
            .pending
            .as_ref()
            .map(|p| p.id)
    }

    pub(super) fn query_design_visible(&self) -> bool {
        self.query_design_request().is_some()
            && self
                .query_design
                .session
                .as_ref()
                .is_some_and(|s| s.mode == Mode::Design)
    }

    pub(super) fn query_design_results_visible(&self) -> bool {
        self.query_design_request().is_some()
            && self
                .query_design
                .session
                .as_ref()
                .is_some_and(|s| s.mode == Mode::Results)
    }

    pub fn query_design_draft(&self) -> Option<&QueryDesignDraft> {
        self.query_design_request()?;
        Some(&self.query_design.session.as_ref()?.draft)
    }

    pub(super) fn query_design_dialog_open(&self) -> bool {
        self.query_design_request().is_some()
            && self
                .query_design
                .session
                .as_ref()
                .is_some_and(|s| s.add_tables)
    }

    pub fn can_describe_query_design(&self, request: &QueryDesignRequest, table: &str) -> bool {
        !self.query_design.has_unresolved_field_inputs()
            && self.query_design_ready()
            && self.query_design_request().as_ref() == Some(request)
            && self.database.as_ref().is_some_and(|database| {
                database
                    .tables
                    .iter()
                    .any(|entry| entry.local && !entry.system && entry.name == table)
            })
            && self.query_design.session.as_ref().is_none_or(|session| {
                session
                    .draft
                    .sources
                    .iter()
                    .any(|source| source.table == table)
                    || session.draft.sources.len() < MAX_DESIGN_SOURCES
            })
    }

    pub fn begin_query_design_description(
        &mut self,
        request: QueryDesignRequest,
        table: String,
        request_id: u64,
    ) -> bool {
        if request_id == 0 || !self.can_describe_query_design(&request, &table) {
            return false;
        }
        self.begin_design_pending(request, request_id, PendingKind::Describe { table });
        true
    }

    pub fn set_query_design_table(
        &mut self,
        request_id: u64,
        metadata: DesignTableMetadata,
    ) -> bool {
        if !self.design_reply_current(request_id) {
            return false;
        }
        let session = self
            .query_design
            .session
            .as_mut()
            .expect("current Design reply");
        let Some(Pending {
            kind: PendingKind::Describe { table },
            ..
        }) = &session.pending
        else {
            return false;
        };
        if metadata.table != *table
            || tablecraft_builders::query::validate_metadata(std::slice::from_ref(&metadata))
                .is_err()
        {
            return false;
        }
        let table = table.as_str();
        // NO_CLONE: Mutate and validate a candidate before replacing the editor: failed metadata
        // must preserve the original draft. Arc/Cow would detach on this first mutation,
        // so they cannot remove this rollback copy; no worker needs to share it.
        let mut candidate = session.draft.clone();
        let added = !candidate
            .sources
            .iter()
            .any(|source| source.table.eq_ignore_ascii_case(table));
        if added {
            let Some(id) = candidate
                .sources
                .iter()
                .map(|source| source.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
            else {
                session.pending = None;
                session.error = Some(self.localizer.text("qbe-limit"));
                return false;
            };
            candidate.sources.push(DesignSource {
                id,
                table: table.to_owned(),
                alias: None,
            });
        }
        let mut descriptors: Vec<_> = session
            .metadata
            .iter()
            .filter(|previous| !previous.table.eq_ignore_ascii_case(table))
            .cloned()
            .collect();
        descriptors.push(metadata);
        if candidate.sources.len() > MAX_DESIGN_SOURCES
            || session.field_inputs.editable_bytes(&candidate).is_err()
            || tablecraft_builders::query::validate_metadata(&descriptors).is_err()
        {
            session.pending = None;
            session.error = Some(self.localizer.text("qbe-limit"));
            return false;
        }
        session.pending = None;
        session.error = None;
        session.draft = candidate;
        session.field_inputs.clear_hints();
        session.metadata = descriptors;
        if added {
            session.revision = session.revision.wrapping_add(1);
        }
        true
    }

    pub fn can_build_query_design(
        &self,
        request: &QueryDesignRequest,
        draft: &QueryDesignDraft,
        _completion: QueryDesignCompletion,
    ) -> bool {
        !self.query_design.has_unresolved_field_inputs()
            && self.query_design_ready()
            && self.query_design_request().as_ref() == Some(request)
            && self.query_design.session.as_ref().is_some_and(|session| {
                session.mode != Mode::Sql && !session.add_tables && &session.draft == draft
            })
    }

    pub fn begin_query_design_build(
        &mut self,
        request: QueryDesignRequest,
        draft: Arc<QueryDesignDraft>,
        completion: QueryDesignCompletion,
        request_id: u64,
    ) -> bool {
        if request_id == 0 || !self.can_build_query_design(&request, &draft, completion) {
            return false;
        }
        self.begin_design_pending(
            request,
            request_id,
            PendingKind::Build { draft, completion },
        );
        true
    }

    pub fn can_import_query_design(&self, request: &QueryDesignRequest, sql: &str) -> bool {
        !self.query_design.has_unresolved_field_inputs()
            && self.query_design_ready()
            && self.query_design_request().as_ref() == Some(request)
            && self.query_design_sql() == Some(sql)
            && sql.len() <= MAX_DESIGN_TEXT_BYTES
    }

    pub fn begin_query_design_import(
        &mut self,
        request: QueryDesignRequest,
        sql: String,
        request_id: u64,
    ) -> bool {
        if request_id == 0 || !self.can_import_query_design(&request, &sql) {
            return false;
        }
        self.begin_design_pending(request, request_id, PendingKind::Import { sql });
        true
    }

    pub fn set_query_design_built(
        &mut self,
        request_id: u64,
        built: BuiltQueryDesign,
        metadata: Vec<DesignTableMetadata>,
    ) -> Option<QueryDesignCompletion> {
        let valid_metadata = tablecraft_builders::query::validate_metadata(&metadata).is_ok()
            && built.draft.sources.iter().all(|source| {
                metadata
                    .iter()
                    .any(|table| table.table.eq_ignore_ascii_case(&source.table))
            });
        if self.query_design.has_unresolved_field_inputs()
            || !self.design_reply_current(request_id)
            || !valid_metadata
            || built.sql.len() > MAX_DESIGN_TEXT_BYTES
            || tablecraft_builders::query::draft_text_bytes(&built.draft).is_err()
        {
            return None;
        }
        let session = self.query_design.session.as_ref()?;
        let completion = match &session.pending.as_ref()?.kind {
            PendingKind::Build { draft, completion } if draft.as_ref() == &built.draft => {
                Some(*completion)
            }
            PendingKind::Import { sql } if self.query_design_sql() == Some(sql.as_str()) => None,
            _ => return None,
        };
        let close_build = session
            .pending
            .as_ref()
            .map(|pending| (pending.request.tab.id, pending.request.draft_revision));
        if !self.replace_query_design_sql(built.sql) {
            return None;
        }
        let session = self.query_design.session.as_mut()?;
        session.pending = None;
        session.error = None;
        session.field_inputs.clear_committed();
        session.draft = built.draft;
        session.metadata = metadata;
        session.revision = session.revision.wrapping_add(1);
        if completion.is_none() {
            session.saved_revision = session.revision;
        }
        session.mode = match completion {
            None => Mode::Design,
            Some(QueryDesignCompletion::ShowSql) => Mode::Sql,
            Some(QueryDesignCompletion::Run) => Mode::Results,
            Some(QueryDesignCompletion::Save | QueryDesignCompletion::SaveExisting) => Mode::Design,
        };
        if session.mode == Mode::Design {
            self.ribbon_tab = super::ribbon::Tab::QueryDesign;
        }
        if matches!(
            completion,
            Some(QueryDesignCompletion::Save | QueryDesignCompletion::SaveExisting)
        ) && let Some((tab_id, revision)) = close_build
        {
            self.query_design_close_built(tab_id, revision);
        }
        if completion == Some(QueryDesignCompletion::Save) && !self.begin_query_creation_name() {
            self.cancel_query_design_close_name();
        }
        completion.or(Some(QueryDesignCompletion::ShowSql))
    }

    pub fn fail_query_design(&mut self, request_id: u64, message: String) {
        if self.design_reply_current(request_id) {
            self.cancel_query_design_close_build();
            let session = self
                .query_design
                .session
                .as_mut()
                .expect("current Design reply");
            session.pending = None;
            session.error = Some(message);
        }
    }

    pub fn cancel_query_design(&mut self, request_id: u64) -> bool {
        if self.query_design_request_id() != Some(request_id) {
            return false;
        }
        self.cancel_query_design_close_build();
        self.query_design
            .session
            .as_mut()
            .expect("pending Design request")
            .pending = None;
        true
    }

    fn begin_design_pending(&mut self, request: QueryDesignRequest, id: u64, kind: PendingKind) {
        let session = self
            .query_design
            .session
            .get_or_insert_with(Default::default);
        session.error = None;
        session.pending = Some(Pending {
            id,
            request,
            publication: self.focus_publication,
            kind,
        });
    }

    fn design_reply_current(&self, id: u64) -> bool {
        self.query_design
            .session
            .as_ref()
            .and_then(|s| s.pending.as_ref())
            .is_some_and(|pending| {
                pending.id == id
                    && pending.publication == self.focus_publication
                    && self.query_design_request().as_ref() == Some(&pending.request)
            })
    }

    pub fn begin_query_design_run(&mut self, request_id: u64) -> bool {
        if request_id == 0
            || self.query_design.has_unresolved_field_inputs()
            || !self.query_design_results_visible()
            || !self.query_design_ready()
        {
            return false;
        }
        let Some(request) = self.query_design_request() else {
            return false;
        };
        // NO_CLONE: Pending results retain the submitted SQL after the editor
        // changes. Capture the owned provenance before installing pending state.
        let Some(sql) = self
            .query_design_sql()
            .and_then(super::query::parameters::copy_text)
        else {
            return false;
        };
        self.begin_design_pending(request, request_id, PendingKind::Run { sql });
        true
    }

    pub fn set_query_design_results(&mut self, request_id: u64, data: super::Data) -> bool {
        if !self.design_reply_current(request_id)
            || data.offset != 0
            || data.has_more
            || data.source_revision.is_some()
            || data.row_ordinals.is_some()
            || data.rows.iter().any(|row| row.len() != data.columns.len())
        {
            return false;
        }
        let valid = self.query_design.session.as_ref().is_some_and(|session| {
            session
                .pending
                .as_ref()
                .is_some_and(|pending| match &pending.kind {
                    PendingKind::Run { sql } => self.query_design_sql() == Some(sql.as_str()),
                    _ => false,
                })
        });
        if !valid {
            return false;
        }
        let pending = self
            .query_design
            .session
            .as_mut()
            .expect("matching Design run")
            .pending
            .take()
            .expect("validated pending run");
        let PendingKind::Run { sql } = pending.kind else {
            unreachable!("validated Design run");
        };
        if !self.set_query_design_data(sql, data) {
            // NO_CLONE: The global diagnostic and independently dismissible Design error both
            // retain this text. Cow cannot borrow mutable UI state; Arc per diagnostic
            // would replace both message contracts for a small exceptional-path copy.
            let message = self
                .error
                .clone()
                .unwrap_or_else(|| self.localizer.text("qbe-invalid-result"));
            if let Some(session) = &mut self.query_design.session {
                session.pending = None;
                session.error = Some(message);
            }
            return false;
        }
        let session = self
            .query_design
            .session
            .as_mut()
            .expect("matching Design run");
        session.pending = None;
        session.error = None;
        session.result_revision = Some(session.revision);
        if let ResultOrigin::Sql(sql) = std::mem::take(&mut self.query_result) {
            self.query_result = ResultOrigin::Design(sql);
        }
        true
    }

    pub(super) fn query_design_results_previous(&self) -> bool {
        self.query_design.session.as_ref().is_some_and(|session| {
            session.mode == Mode::Results
                && (session.result_revision != Some(session.revision)
                    || self.busy
                    || self.error.is_some()
                    || session.error.is_some())
        })
    }

    pub(in crate::workspace) fn show_query_design_tables(&mut self) {
        if self.query_design_ready() && self.query_design_visible() {
            self.query_design
                .session
                .as_mut()
                .expect("Design view")
                .add_tables = true;
        }
    }

    pub(super) fn show_query_design(&mut self) -> Option<Action> {
        if !self.query_design_ready() {
            return None;
        }
        let request = self.query_design_request()?;
        if self.query_design_results_visible() || self.query_design_sql()?.trim().is_empty() {
            let session = self
                .query_design
                .session
                .get_or_insert_with(Default::default);
            session.mode = Mode::Design;
            self.ribbon_tab = super::ribbon::Tab::QueryDesign;
            session.add_tables =
                session.draft.sources.is_empty() && session.draft.columns.is_empty();
            None
        } else if !self.query_design_visible() {
            Some(Action::ImportQueryDesign {
                request,
                // NO_CLONE: Import owns the submitted SQL while the editor keeps its mutable buffer
                // after rejection. Rc cannot cross the worker; borrowing the editor
                // through Cow cannot survive asynchronous dispatch.
                sql: super::query::parameters::copy_text(self.query_design_sql()?)?,
            })
        } else {
            None
        }
    }

    pub(super) fn show_query_design_sql(&mut self) -> Option<Action> {
        if self.query_design.has_unresolved_field_inputs() || !self.query_design_ready() {
            return None;
        }
        if self.query_design_results_visible() {
            self.query_design.session.as_mut()?.mode = Mode::Sql;
            return None;
        }
        let request = self.query_design_request()?;
        let session = self.query_design.session.as_ref()?;
        (session.mode == Mode::Design).then(|| Action::BuildQueryDesign {
            request,
            // NO_CLONE: Capture a stable submitted snapshot while the editor keeps its mutable
            // draft. Arc then shares this one immutable payload with UI pending and the
            // worker; Rc cannot cross the worker thread and Cow cannot borrow mutable
            // editor state.
            draft: Arc::new(session.draft.clone()),
            completion: QueryDesignCompletion::ShowSql,
        })
    }

    pub(in crate::workspace) fn query_design_save_action(&self) -> Option<Action> {
        self.query_design_save_action_for(false)
    }

    pub(in crate::workspace) fn query_design_save_action_for(
        &self,
        existing: bool,
    ) -> Option<Action> {
        let request = self.query_design_request()?;
        let session = self.query_design.session.as_ref()?;
        let completion = if existing {
            QueryDesignCompletion::SaveExisting
        } else {
            QueryDesignCompletion::Save
        };
        self.can_build_query_design(&request, &session.draft, completion)
            .then(|| Action::BuildQueryDesign {
                request,
                // NO_CLONE: The worker and live editable Design need independent snapshots;
                // this one paid immutable Arc is shared by the pending request and worker.
                draft: Arc::new(session.draft.clone()),
                completion,
            })
    }

    pub(super) fn query_design_run_action(&self) -> Option<Action> {
        let request = self.query_design_request()?;
        let session = self.query_design.session.as_ref()?;
        self.can_build_query_design(&request, &session.draft, QueryDesignCompletion::Run)
            .then(|| Action::BuildQueryDesign {
                request,
                // NO_CLONE: Capture a stable submitted snapshot while the editor keeps its mutable
                // draft. Arc then shares this one immutable payload with UI pending and
                // the worker; Rc cannot cross the worker thread and Cow cannot borrow
                // mutable editor state.
                draft: Arc::new(session.draft.clone()),
                completion: QueryDesignCompletion::Run,
            })
    }
}

#[cfg(test)]
mod tests;

impl QueryDesign {
    pub(in crate::workspace) fn save_revision(&self) -> Option<u64> {
        let session = self.session.as_ref()?;
        (session.mode == Mode::Design).then_some(session.revision)
    }
}

impl QueryDesign {
    pub(in crate::workspace) fn dirty_revision(&self) -> Option<u64> {
        let session = self.session.as_ref()?;
        (session.revision != session.saved_revision).then_some(session.revision)
    }

    pub(in crate::workspace) fn is_dirty(&self) -> bool {
        self.dirty_revision().is_some()
    }

    pub(in crate::workspace) fn mark_saved(&mut self) {
        if let Some(session) = &mut self.session {
            session.saved_revision = session.revision;
        }
    }
}

impl QueryDesign {
    pub(in crate::workspace) fn revision(&self) -> Option<u64> {
        Some(self.session.as_ref()?.revision)
    }
}
