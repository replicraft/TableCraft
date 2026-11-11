use super::{Command, Error, Response, document::Document};
use crate::{
    task_signal,
    thread_start::{Spawner, ThreadSpawner},
};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, TryRecvError, sync_channel},
};

use tablecraft_expressions::functions::dates::clock::Clock;

const NO_REQUEST: u64 = 0;
const EXHAUSTED_GENERATION: u64 = u64::MAX;

/// This worker has permanently exhausted its monotonic read-request identities.
/// Create a fresh worker and explicitly reopen the source before requesting more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("read request identities exhausted; restart the worker and reopen the database")]
pub struct ReadRequestExhausted;

type Reply = (u64, Result<Response, Error>);

struct Pending {
    id: u64,
    command: Command,
}

struct Shared {
    generation: AtomicU64,
    active: AtomicU64,
    liveness: Mutex<Receiver<Infallible>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
    stopped: AtomicBool,
    pending: Mutex<Option<Pending>>,
    ready: task_signal::Signal,
    response: Mutex<Option<Reply>>,
}

impl Shared {
    fn issue_generation(&self) -> Result<u64, ReadRequestExhausted> {
        self.generation
            .try_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
                generation
                    .checked_add(1)
                    .filter(|next| *next < EXHAUSTED_GENERATION)
            })
            .map(|previous| previous + 1)
            .map_err(|_| ReadRequestExhausted)
    }

    fn revoke_generation(&self) -> u64 {
        // The final value is cancellation-only, never an issued request ID.
        self.generation
            .try_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
                Some(generation.saturating_add(1))
            })
            .expect("revocation always supplies a value")
            .saturating_add(1)
    }

    fn disconnected(&self) -> bool {
        match self
            .liveness
            .lock()
            .expect("liveness receiver has no panicking callback")
            .try_recv()
        {
            Err(TryRecvError::Disconnected) => true,
            Err(TryRecvError::Empty) => false,
            Ok(never) => match never {},
        }
    }

    fn repaint(&self) {
        if catch_unwind(AssertUnwindSafe(|| (self.repaint)())).is_err() {
            tracing::error!("read completion callback panicked");
        }
    }

    // Only after terminal disconnection: never recover or execute a poisoned
    // mailbox. Its remaining payload is dropped when the failed worker is replaced.
    fn discard_dead_mailboxes(&self) {
        let pending = match self.pending.lock() {
            Ok(mut pending) => pending.take(),
            Err(_) => None,
        };
        let response = match self.response.lock() {
            Ok(mut response) => response.take(),
            Err(_) => None,
        };
        drop(pending);
        drop(response);
    }
}

/// One worker, one replaceable pending request and one replaceable owned response.
/// Native sources and query caches never cross the thread boundary.
pub struct Worker {
    shared: Arc<Shared>,
}

impl Worker {
    /// Start a dedicated native read actor. Startup failure is delivered by
    /// `poll` for an accepted request rather than executing work on the caller.
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
            Command::InspectDatabaseOptions { document_revision } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_database_options(*document_revision, cancelled),
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
            Command::InspectTableDropSource {
                document_revision,
                table,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_table_drop_source(*document_revision, table, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::ReviewTableDrop {
                document_revision,
                table,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_table_drop(*document_revision, table, cancelled),
            #[cfg(feature = "offline-edit")]
            Command::OpenTableDropped { witness } => {
                let fresh_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_table_dropped(witness, fresh_revision, cancelled)?;
                document = Some(opened);
                document_revision = fresh_revision;
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
            Command::ReviewFieldDrop {
                document_revision,
                table,
                column,
                native_number,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_field_drop(*document_revision, table, column, *native_number, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::OpenFieldDropped { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_field_dropped(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::InspectFieldDropSource {
                document_revision,
                table,
                column,
                native_number,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_field_drop_source(
                    *document_revision,
                    table,
                    column,
                    *native_number,
                    cancelled,
                ),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewFieldRename {
                document_revision,
                table,
                column,
                native_number,
                new_name,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_field_rename(
                    *document_revision,
                    table,
                    column,
                    *native_number,
                    new_name,
                    cancelled,
                ),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::OpenFieldRenamed { witness } => {
                document = None;
                document_revision = document_revision
                    .checked_add(1)
                    .ok_or(Error::RevisionExhausted)?;
                let (opened, response) =
                    Document::open_field_renamed(witness, document_revision, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::InspectFieldRenameSource {
                document_revision,
                table,
                column,
                native_number,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_field_rename_source(
                    *document_revision,
                    table,
                    column,
                    *native_number,
                    cancelled,
                ),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::InspectFieldAdditionAvailability {
                document_revision,
                table,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .inspect_field_addition_availability(*document_revision, table, cancelled),
            #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
            Command::ReviewAddField {
                document_revision,
                table,
                column,
                declaration,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_add_field(*document_revision, table, column, *declaration, cancelled),
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
                .build_query_design(target, draft.as_ref(), cancelled),
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
            Command::InspectQueryCollection { result, binding } => {
                if result.document_revision() != document_revision {
                    return Err(Error::StaleDocument);
                }
                document
                    .as_ref()
                    .ok_or(Error::NoDatabase)?
                    .inspect_query_collection(result, *binding, cancelled)
            }
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
            Command::DiscoverQueryParameters { target, options } => {
                // NO_CLONE: Command and issued discovery independently retain the
                // immutable syntax witness; share its Arc without copying SQL.
                let target = Arc::clone(target);
                document
                    .as_mut()
                    .ok_or(Error::NoDatabase)?
                    .discover_query_parameters(target, *options, cancelled)
            }
            Command::RunDiscoveredQuery {
                target,
                parameters,
                options,
            } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .run_discovered_query(target, parameters, *options, clock.as_ref(), cancelled),
            Command::RunQuery {
                target,
                parameters,
                options,
            } => document.as_mut().ok_or(Error::NoDatabase)?.run_query(
                target,
                parameters,
                *options,
                clock.as_ref(),
                cancelled,
            ),
            #[cfg(feature = "offline-edit")]
            Command::ReviewSchemaSql { target } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .review_schema_sql(target, cancelled),
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
            Command::RunSavedQuery {
                target,
                parameters,
                options,
            } => document.as_mut().ok_or(Error::NoDatabase)?.run_saved_query(
                target,
                parameters,
                *options,
                clock.as_ref(),
                cancelled,
            ),
            Command::Query { sql, options } => document.as_mut().ok_or(Error::NoDatabase)?.query(
                sql,
                *options,
                clock.as_ref(),
                cancelled,
            ),
        })
    }

    fn start(
        repaint: impl Fn() + Send + Sync + 'static,
        execute: impl FnMut(&mut Command, &dyn Fn() -> bool) -> Result<Response, Error> + Send + 'static,
    ) -> Self {
        Self::start_with_spawner(repaint, execute, ThreadSpawner)
    }

    fn start_with_spawner(
        repaint: impl Fn() + Send + Sync + 'static,
        execute: impl FnMut(&mut Command, &dyn Fn() -> bool) -> Result<Response, Error> + Send + 'static,
        spawner: impl Spawner,
    ) -> Self {
        let (alive, liveness) = sync_channel::<Infallible>(0);
        let (ready, wake) = task_signal::channel();
        let shared = Arc::new(Shared {
            generation: AtomicU64::new(0),
            active: AtomicU64::new(NO_REQUEST),
            liveness: Mutex::new(liveness),
            repaint: Arc::new(repaint),
            stopped: AtomicBool::new(false),
            pending: Mutex::new(None),
            ready,
            response: Mutex::new(None),
        });
        let thread_shared = Arc::clone(&shared);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let started = spawner.spawn(move || {
            let actor_shared = Arc::clone(&thread_shared);
            // A panic is terminal. The owned executor/document is dropped before
            // disconnecting; no possibly invalid cache is resumed afterward.
            let result = catch_unwind(AssertUnwindSafe(move || {
                let mut execute = execute;
                let _subscriber = tracing::dispatcher::set_default(&dispatch);
                while wake.wait() {
                    let mut pending = actor_shared
                        .pending
                        .lock()
                        .expect("read request mailbox poisoned");
                    if actor_shared.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let Some(mut request) = pending.take() else {
                        continue;
                    };
                    drop(pending);

                    let cancelled = || {
                        actor_shared.stopped.load(Ordering::Acquire)
                            || actor_shared.generation.load(Ordering::Acquire) != request.id
                    };
                    let operation = match &request.command {
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::InspectDatabaseOptions { .. } => "inspect_database_options",
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
                        Command::InspectTableDropSource { .. } => "inspect_table_drop_source",
                        #[cfg(feature = "offline-edit")]
                        Command::ReviewTableDrop { .. } => "review_table_drop",
                        #[cfg(feature = "offline-edit")]
                        Command::OpenTableDropped { .. } => "open_table_dropped",
                        #[cfg(feature = "offline-edit")]
                        Command::OpenInsertOriginal { .. } => "open_insert_original",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::ReviewTableCreation { .. } => "review_table_creation",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::ReviewFieldRename { .. } => "review_field_rename",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::ReviewFieldDrop { .. } => "review_field_drop",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::OpenFieldRenamed { .. } => "open_field_renamed",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::OpenFieldDropped { .. } => "open_field_dropped",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::InspectFieldRenameSource { .. } => "inspect_field_rename_source",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::InspectFieldDropSource { .. } => "inspect_field_drop_source",
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::InspectFieldAdditionAvailability { .. } => {
                            "inspect_field_addition_source"
                        }
                        #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
                        Command::ReviewAddField { .. } => "review_add_field",
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
                        Command::InspectQueryCollection { .. } => "inspect_query_collection",
                        Command::Query { .. } => "query",
                        Command::InspectQuery { .. } => "inspect_query",
                        #[cfg(feature = "offline-edit")]
                        Command::ReviewQueryUpdate { .. } => "review_query_update",
                        #[cfg(feature = "offline-edit")]
                        Command::ReviewQueryDelete { .. } => "review_query_delete",
                        #[cfg(feature = "offline-edit")]
                        Command::ReviewSchemaSql { .. } => "review_schema_sql",
                        #[cfg(feature = "offline-edit")]
                        Command::ReviewQueryInsert { .. } => "review_query_insert",
                        Command::DiscoverQueryParameters { .. } => "discover_query_parameters",
                        Command::RunDiscoveredQuery { .. } => "run_discovered_query",
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
                        let replaced = actor_shared
                            .response
                            .lock()
                            .expect("read reply mailbox poisoned")
                            .replace((request.id, result));
                        drop(replaced);
                        actor_shared.repaint();
                    }
                }
            }));
            drop(alive);
            if result.is_err() && !thread_shared.stopped.load(Ordering::Acquire) {
                thread_shared.repaint();
            }
        });
        if started.is_err() {
            tracing::error!("read worker could not start");
        }
        Self { shared }
    }

    /// Supersede earlier work without waiting for that work to finish.
    /// A disconnected executor refuses execution and publishes ReadWorkerStopped
    /// for this request; it never resumes a document after an actor panic.
    ///
    /// # Errors
    /// Returns [`ReadRequestExhausted`] without replacing earlier work when no
    /// fresh identity remains. The rejected command is dropped. Exhaustion is
    /// permanent; cancellation can still revoke the final accepted request.
    ///
    /// # Panics
    /// Panics if an internal live mailbox was poisoned. Poison is an invariant
    /// failure, not permission to resume a potentially invalid document.
    pub fn request(&self, command: Command) -> Result<u64, ReadRequestExhausted> {
        // The worker cannot take this command until its preceding response has
        // been cleared. Serializing generation issuance with replacement also
        // prevents a slower caller from replacing a newer caller's command.
        if self.shared.disconnected() {
            let id = self.shared.issue_generation()?;
            self.shared.active.fetch_max(id, Ordering::AcqRel);
            drop(command);
            self.shared.repaint();
            return Ok(id);
        }
        let mut pending = self
            .shared
            .pending
            .lock()
            .expect("read request mailbox poisoned");
        let id = self.shared.issue_generation()?;
        self.shared.active.fetch_max(id, Ordering::AcqRel);
        let old_response = self
            .shared
            .response
            .lock()
            .expect("read reply mailbox poisoned")
            .take();
        let old_pending = pending.replace(Pending { id, command });
        drop(pending);
        self.shared.ready.notify();
        drop(old_response);
        drop(old_pending);
        Ok(id)
    }

    /// Whether this exact readonly request still owns reply publication.
    /// Superseding or cancelling revokes publication immediately; native work
    /// may still be checking cancellation. This says nothing about SaveWorker.
    pub fn publication_is_current(&self, id: u64) -> bool {
        id != NO_REQUEST
            && id < EXHAUSTED_GENERATION
            && self.shared.generation.load(Ordering::Acquire) == id
    }

    /// Take the current reply, or the exact accepted request's terminal actor error.
    ///
    /// # Panics
    /// Panics if a live reply mailbox or the liveness guard is poisoned.
    pub fn poll(&self) -> Option<Reply> {
        if self.shared.disconnected() {
            let id = self.shared.active.swap(NO_REQUEST, Ordering::AcqRel);
            if id == NO_REQUEST || id != self.shared.generation.load(Ordering::Acquire) {
                return None;
            }
            self.shared.discard_dead_mailboxes();
            return Some((id, Err(Error::ReadWorkerStopped)));
        }
        let response = self
            .shared
            .response
            .lock()
            .expect("read reply mailbox poisoned")
            .take()?;
        if response.0 != self.shared.generation.load(Ordering::Acquire)
            || self
                .shared
                .active
                .compare_exchange(response.0, NO_REQUEST, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }
        Some(response)
    }

    /// Revoke publication without joining the actor or waiting for native I/O.
    /// This remains effective after request identities are exhausted.
    ///
    /// # Panics
    /// Panics if a live mailbox or the liveness guard is poisoned.
    pub fn cancel(&self) {
        if self.shared.disconnected() {
            let revoked = self.shared.revoke_generation();
            self.shared
                .active
                .try_update(Ordering::AcqRel, Ordering::Acquire, |id| {
                    (id <= revoked).then_some(NO_REQUEST)
                })
                .ok();
            self.shared.discard_dead_mailboxes();
            return;
        }
        let mut pending = self
            .shared
            .pending
            .lock()
            .expect("read request mailbox poisoned");
        let revoked = self.shared.revoke_generation();
        self.shared
            .active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |id| {
                (id <= revoked).then_some(NO_REQUEST)
            })
            .ok();
        let old_pending = pending.take();
        let old_response = self
            .shared
            .response
            .lock()
            .expect("read reply mailbox poisoned")
            .take();
        drop(pending);
        drop(old_pending);
        drop(old_response);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        self.shared
            .generation
            .store(EXHAUSTED_GENERATION, Ordering::Release);
        self.shared.active.store(NO_REQUEST, Ordering::Release);
        self.shared.ready.notify();
        // No explicit mailbox lock or join here. A live actor reclaims its
        // owned source/queued data after any blocking read returns.
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

#[cfg(test)]
mod liveness_tests;

#[cfg(test)]
mod generation_tests;
