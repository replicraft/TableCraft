use super::{Command, Error, Response, document::Document};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use tablecraft_expressions::functions::dates::clock::Clock;

type Reply = (u64, Result<Response, Error>);

struct Pending {
    id: u64,
    command: Command,
}

struct Shared {
    generation: AtomicU64,
    stopped: AtomicBool,
    pending: Mutex<Option<Pending>>,
    ready: Condvar,
    response: Mutex<Option<Reply>>,
}

/// One worker, one replaceable pending request and one replaceable owned response.
/// Native sources and query caches never cross the thread boundary.
pub struct Worker {
    shared: Arc<Shared>,
}

impl Worker {
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_query_clock(repaint, Arc::new(crate::query::context::LocalClock))
    }

    /// Use a caller-owned retrieval clock. The provider is read only by executed
    /// clock expressions, never by open, inspection, or parameter discovery.
    /// It must select its own timezone and report failures without a fallback.
    pub fn with_query_clock(
        repaint: impl Fn() + Send + Sync + 'static,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let mut document: Option<Document> = None;
        let mut document_revision = 0_u64;
        Self::start(repaint, move |command, cancelled| match command {
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewDisableNameAutoCorrect { document_revision } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_disable_name_auto_correct(*document_revision, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::OpenDatabaseOptionsSaved { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_database_options_saved(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(feature = "offline-edit")]
            Command::ReviewTableRename {
                document_revision,
                table,
                new_name,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_table_rename(*document_revision, table, new_name, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::InspectQuerySqlDraft { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_query_sql_draft(target, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewQuerySql {
                target,
                original_sql,
                proposed_sql,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_query_sql(target, original_sql, proposed_sql, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewQueryLiteralExpression {
                target,
                expected,
                replacement,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_query_literal_expression(target, expected, replacement, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::OpenQueryExpressionSaved { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_query_expression_saved(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(feature = "offline-edit")]
            Command::OpenTableRenamed { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_table_renamed(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(feature = "offline-edit")]
            Command::OpenInsertOriginal { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_insert_original(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::InspectNumberColumnAvailability {
                document_revision,
                table,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_number_column_availability(*document_revision, table, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewAddNumberColumn {
                document_revision,
                table,
                column,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_add_number_column(*document_revision, table, column, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewAddInt32Column {
                document_revision,
                table,
                column,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_add_int32_column(*document_revision, table, column, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::OpenColumnAdded { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_column_added(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewTableCreation {
                document_revision,
                snapshot,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_table_creation(*document_revision, snapshot, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::OpenTableCreated { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_table_created(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(feature = "ace16-create")]
            Command::OpenCreated { report } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_created(report, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            Command::Open { path, password } => {
                // A failed open cannot leave the old file behind a new UI title.
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open(path, password, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            Command::DescribeTotals { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .describe_totals(target, cancelled),
            Command::Totals {
                target,
                specs,
                base,
            } => document.as_mut().ok_or(Error::NoDatabase)?.totals(
                target,
                specs,
                base.as_ref(),
                cancelled,
            ),
            Command::DescribeQueryDesign { target, table } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .describe_query_design(target, table, cancelled),
            Command::BuildQueryDesign { target, draft } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .build_query_design(target, draft, cancelled),
            Command::ImportQueryDesign { target, sql } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .import_query_design(target, sql, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReplaceFindMatch {
                request,
                found,
                replacement,
                filter,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .replace_find_match(
                    request,
                    found,
                    replacement,
                    filter.as_ref().map(|(handle, enabled)| (handle, *enabled)),
                    cancelled,
                ),
            Command::Page { table, offset } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .page(table, *offset, cancelled),
            Command::Describe { table } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .describe(table, cancelled),
            Command::InspectCollection { target } => {
                // Failed opens also advance the attempt revision. Reject that
                // old target before catalog/page I/O, even with no live Document.
                if target.document_revision != document_revision {
                    return Err(Error::StaleDocument);
                }
                document
                    .as_mut()
                    .ok_or(Error::NoDatabase)?
                    .inspect_collection(target, cancelled)
            }
            #[cfg(feature = "offline-edit")]
            Command::BeginEdit { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .begin_edit(target, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewEdit { draft_id, input } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_edit(*draft_id, input, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::BeginInsert { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .begin_insert(target, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewInsert { draft_id, inputs } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_insert(*draft_id, inputs, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewDelete { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_delete(target, cancelled),
            Command::SortView {
                target,
                sort,
                base,
                enabled,
            } => document.as_mut().ok_or(Error::NoDatabase)?.sort_view(
                target,
                sort.as_ref(),
                base.as_ref(),
                *enabled,
                cancelled,
            ),
            Command::ClearFilter { handle } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .clear_filter(handle, cancelled),
            Command::ReapplyFilter {
                definition,
                document_revision,
                enabled,
                offset,
            } => document.as_mut().ok_or(Error::NoDatabase)?.reapply_filter(
                definition,
                *document_revision,
                *enabled,
                *offset,
                cancelled,
            ),
            Command::DescribeFieldFilter { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .describe_field_filter(target, cancelled),
            Command::ApplyFieldFilter {
                target,
                operator,
                inputs,
                base,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .apply_field_filter(target, *operator, inputs, base.as_ref(), cancelled),
            Command::FilterSelection {
                target,
                comparison,
                base,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .filter_selection(target, *comparison, base.as_ref(), cancelled),
            Command::FilterPage {
                handle,
                offset,
                enabled,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .filter_page(handle, *offset, *enabled, cancelled),
            Command::FindFiltered {
                handle,
                request,
                enabled,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .find_filtered(handle, request, *enabled, cancelled),
            Command::FindNext { request } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .find_next(request, cancelled),
            Command::InspectQuery {
                document_revision,
                sql,
            } => document.as_ref().ok_or(Error::NoDatabase)?.inspect_query(
                *document_revision,
                sql,
                cancelled,
            ),
            Command::RunQuery { target, parameters } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .run_query(target, parameters, clock.as_ref(), cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewQueryInsert { target, parameters } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_query_insert(target, parameters, clock.as_ref(), cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewQueryDelete { target, parameters } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_query_delete(target, parameters, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewQueryUpdate { target, parameters } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_query_update(target, parameters, cancelled),
            Command::InspectSavedQuery { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_saved_query(target, cancelled),
            Command::RunSavedQuery { target, parameters } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .run_saved_query(target, parameters, clock.as_ref(), cancelled),
            Command::Query { sql } => {
                document
                    .as_mut()
                    .ok_or(Error::NoDatabase)?
                    .query(sql, clock.as_ref(), cancelled)
            }
        })
    }

    fn start(
        repaint: impl Fn() + Send + Sync + 'static,
        mut execute: impl FnMut(&mut Command, &dyn Fn() -> bool) -> Result<Response, Error>
        + Send
        + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            pending: Mutex::new(None),
            ready: Condvar::new(),
            response: Mutex::new(None),
        });
        let thread_shared = Arc::clone(&shared);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::spawn(move || {
            let _subscriber = tracing::dispatcher::set_default(&dispatch);
            loop {
                let mut pending = thread_shared.pending.lock().unwrap();
                while pending.is_none() && !thread_shared.stopped.load(Ordering::Acquire) {
                    pending = thread_shared.ready.wait(pending).unwrap();
                }
                if thread_shared.stopped.load(Ordering::Acquire) {
                    break;
                }
                let Some(mut request) = pending.take() else {
                    continue;
                };
                drop(pending);

                let cancelled = || {
                    thread_shared.stopped.load(Ordering::Acquire)
                        || thread_shared.generation.load(Ordering::Acquire) != request.id
                };
                let operation = match &request.command {
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::ReviewDisableNameAutoCorrect { .. } => "review_database_options",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::OpenDatabaseOptionsSaved { .. } => "open_database_options_saved",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewTableRename { .. } => "review_table_rename",
                    #[cfg(feature = "offline-edit")]
                    Command::InspectQuerySqlDraft { .. } => "inspect_query_sql_draft",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewQuerySql { .. } => "review_query_sql",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewQueryLiteralExpression { .. } => "review_query_expression",
                    #[cfg(feature = "offline-edit")]
                    Command::OpenQueryExpressionSaved { .. } => "open_query_expression_saved",
                    #[cfg(feature = "offline-edit")]
                    Command::OpenTableRenamed { .. } => "open_table_renamed",
                    #[cfg(feature = "offline-edit")]
                    Command::OpenInsertOriginal { .. } => "open_insert_original",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::ReviewTableCreation { .. } => "review_table_creation",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::InspectNumberColumnAvailability { .. } => {
                        "inspect_number_column_source"
                    }
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::ReviewAddNumberColumn { .. } => "review_add_number_column",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::ReviewAddInt32Column { .. } => "review_add_column",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::OpenColumnAdded { .. } => "open_column_added",
                    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                    Command::OpenTableCreated { .. } => "open_table_created",
                    Command::Open { .. } => "open",
                    #[cfg(feature = "ace16-create")]
                    Command::OpenCreated { .. } => "open_created",
                    #[cfg(feature = "offline-edit")]
                    Command::BeginEdit { .. } => "begin_edit",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewEdit { .. } => "review_edit",
                    #[cfg(feature = "offline-edit")]
                    Command::BeginInsert { .. } => "begin_insert",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewInsert { .. } => "review_insert",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewDelete { .. } => "review_delete",
                    Command::SortView { .. } => "sort_view",
                    Command::ClearFilter { .. } => "clear_filter",
                    Command::ReapplyFilter { .. } => "reapply_filter",
                    Command::DescribeQueryDesign { .. } => "describe_query_design",
                    Command::BuildQueryDesign { .. } => "build_query_design",
                    Command::ImportQueryDesign { .. } => "import_query_design",
                    #[cfg(feature = "offline-edit")]
                    Command::ReplaceFindMatch { .. } => "replace_find_match",
                    Command::DescribeTotals { .. } => "describe_totals",
                    Command::Totals { .. } => "totals",
                    Command::DescribeFieldFilter { .. } => "describe_field_filter",
                    Command::ApplyFieldFilter { .. } => "apply_field_filter",
                    Command::FilterSelection { .. } => "filter_selection",
                    Command::FilterPage { .. } => "filter_page",
                    Command::FindFiltered { .. } => "find_filtered",
                    Command::FindNext { .. } => "find_next",
                    Command::Page { .. } => "page",
                    Command::Describe { .. } => "describe",
                    Command::InspectCollection { .. } => "inspect_collection",
                    Command::Query { .. } => "query",
                    Command::InspectQuery { .. } => "inspect_query",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewQueryUpdate { .. } => "review_query_update",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewQueryDelete { .. } => "review_query_delete",
                    #[cfg(feature = "offline-edit")]
                    Command::ReviewQueryInsert { .. } => "review_query_insert",
                    Command::RunQuery { .. } => "run_query",
                    Command::InspectSavedQuery { .. } => "inspect_saved_query",
                    Command::RunSavedQuery { .. } => "run_saved_query",
                };
                let span = tracing::debug_span!("application read task", operation);
                let _entered = span.enter();
                let result = execute(&mut request.command, &cancelled);
                let superseded = cancelled();
                tracing::debug!(
                    success = result.is_ok(),
                    superseded,
                    "application read task completed"
                );
                if !superseded {
                    *thread_shared.response.lock().unwrap() = Some((request.id, result));
                    if catch_unwind(AssertUnwindSafe(&repaint)).is_err() {
                        tracing::error!("read completion callback panicked");
                    }
                }
            }
        });
        Self { shared }
    }

    /// Supersede earlier work without waiting for that work to finish.
    pub fn request(&self, command: Command) -> u64 {
        // The worker cannot take this command until its preceding response has
        // been cleared. Serializing generation issuance with replacement also
        // prevents a slower caller from replacing a newer caller's command.
        let mut pending = self.shared.pending.lock().unwrap();
        let id = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.shared.response.lock().unwrap().take();
        *pending = Some(Pending { id, command });
        drop(pending);
        self.shared.ready.notify_one();
        id
    }

    pub fn poll(&self) -> Option<Reply> {
        let response = self.shared.response.lock().unwrap().take()?;
        (response.0 == self.shared.generation.load(Ordering::Acquire)).then_some(response)
    }

    pub fn cancel(&self) {
        let mut pending = self.shared.pending.lock().unwrap();
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        pending.take();
        self.shared.response.lock().unwrap().take();
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        self.cancel();
        self.shared.ready.notify_one();
        // No join on the GUI thread: a currently blocking filesystem call may
        // return later. The owned worker closes its source when it observes stop.
    }
}

#[cfg(test)]
impl Worker {
    pub(super) fn test_worker(
        execute: impl FnMut(&mut Command, &dyn Fn() -> bool) -> Result<Response, Error> + Send + 'static,
    ) -> Self {
        Self::start(|| {}, execute)
    }
}
