//! The actual offline owner lives on this worker until released or diagnosed.
mod apply;
#[cfg(feature = "ace16-create")]
mod apply_add_column;
#[cfg(feature = "ace16-create")]
mod apply_database_options;
mod apply_delete;
#[cfg(feature = "ace16-create")]
mod apply_field_addition;
#[cfg(feature = "ace16-create")]
mod apply_field_drop;
#[cfg(feature = "ace16-create")]
mod apply_field_rename;
#[cfg(feature = "ace16-create")]
mod apply_index_creation;
#[cfg(feature = "ace16-create")]
mod apply_index_drop;
mod apply_insert;
mod apply_query_action;
mod apply_query_expression;
mod apply_query_sql;
#[cfg(feature = "ace16-create")]
mod apply_table_creation;
mod apply_table_drop;
mod apply_table_rename;
mod outcomes;
mod restart;

#[cfg(feature = "ace16-create")]
use self::apply_add_column::apply_add_column;
#[cfg(feature = "ace16-create")]
use self::apply_database_options::apply_database_options;
#[cfg(feature = "ace16-create")]
use self::apply_index_creation::apply_index_creation;
#[cfg(feature = "ace16-create")]
use self::apply_index_drop::apply_index_drop;
#[cfg(feature = "ace16-create")]
use self::apply_table_creation::apply_table_creation;
use self::{
    apply::apply,
    apply_delete::apply_delete,
    apply_insert::apply_insert,
    apply_query_action::apply_query_action,
    apply_query_expression::apply_query_expression,
    apply_query_sql::apply_query_sql,
    apply_table_rename::apply_table_rename,
    outcomes::{finish, known, no_session, receipt, retain},
    restart::restart,
};
use crate::editing::save::{
    SaveFailureKind, SaveOperation, SaveOutcome, SaveReport, SaveStartError, SaveStartFailure,
};
#[cfg(feature = "ace16-create")]
use crate::editing::{
    ReviewedAddColumn, ReviewedDatabaseOptions, ReviewedFieldAddition, ReviewedFieldDrop,
    ReviewedFieldRename, ReviewedIndexCreation, ReviewedIndexDrop, ReviewedNumberColumn,
    ReviewedTableCreation,
};
use crate::{
    controls::action_recovery_budget,
    editing::{
        ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryDelete, ReviewedQueryExpression,
        ReviewedQueryInsert, ReviewedQuerySql, ReviewedQueryUpdate, ReviewedTableDrop,
        ReviewedTableRename, platform::query_action::Plan,
    },
};
use crate::{
    task_signal,
    thread_start::{Spawner, ThreadSpawner},
};
use std::{
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
};
use tablecraft_access::editing::offline::{Failure, OfflineWriteSession};

enum Task {
    #[cfg(test)]
    Panic(health_tests::PanicTask),
    #[cfg(feature = "ace16-create")]
    ApplyDatabaseOptions(ReviewedDatabaseOptions, PathBuf),
    ApplyQueryExpression(ReviewedQueryExpression, PathBuf),
    ApplyQuerySql(ReviewedQuerySql, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyAddColumn(ReviewedAddColumn, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyNumberColumn(ReviewedNumberColumn, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyFieldAddition(ReviewedFieldAddition, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyFieldRename(ReviewedFieldRename, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyFieldDrop(ReviewedFieldDrop, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyTableCreation(ReviewedTableCreation, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyIndexCreation(ReviewedIndexCreation, PathBuf),
    #[cfg(feature = "ace16-create")]
    ApplyIndexDrop(ReviewedIndexDrop, PathBuf),
    Apply(ReviewedEdit, PathBuf),
    ApplyTableRename(ReviewedTableRename, PathBuf),
    ApplyTableDrop(ReviewedTableDrop, PathBuf),
    ApplyInsert(ReviewedInsert, PathBuf),
    ApplyDelete(ReviewedDelete, PathBuf),
    ApplyQueryUpdate(ReviewedQueryUpdate, PathBuf),
    ApplyQueryDelete(ReviewedQueryDelete, PathBuf),
    ApplyQueryInsert(ReviewedQueryInsert, PathBuf),
    Restart(PathBuf, PathBuf),
    Recover,
    Release,
}

enum RetainedOwner {
    Failure(Failure),
    Session(OfflineWriteSession),
}

impl RetainedOwner {
    fn into_session(self) -> OfflineWriteSession {
        match self {
            Self::Failure(failure) => failure.into_session(),
            Self::Session(session) => session,
        }
    }
}

struct Mailbox {
    next: u64,
    active: Option<SaveOperation>,
    validity: Option<Arc<AtomicBool>>,
    previous: Option<SaveReport>,
    retained: bool,
    pending: Option<(u64, Task)>,
    report: Option<SaveReport>,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    liveness: Mutex<Receiver<Infallible>>,
    ready: task_signal::Signal,
    cancelled: AtomicBool,
    stopped: AtomicBool,
}

/// First actor local: resource teardown precedes disconnection and notification.
struct ActorExit {
    alive: Option<SyncSender<Infallible>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl Drop for ActorExit {
    fn drop(&mut self) {
        drop(self.alive.take());
        if catch_unwind(AssertUnwindSafe(|| (self.repaint)())).is_err() {
            tracing::error!("save actor exit callback panicked");
        }
    }
}

/// The native executor invalidates its actual captured source on abnormal exit.
/// No locks, callbacks or resource cleanup are attempted by this guard.
struct ActorSource {
    validity: Option<Arc<AtomicBool>>,
    graceful: bool,
}

impl Drop for ActorSource {
    fn drop(&mut self) {
        if (!self.graceful || std::thread::panicking())
            && let Some(validity) = &self.validity
        {
            validity.store(false, Ordering::Release);
        }
    }
}

impl Shared {
    fn disconnected(&self) -> bool {
        match self
            .liveness
            .lock()
            .expect("save liveness channel poisoned")
            .try_recv()
        {
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => true,
            Ok(never) => match never {},
        }
    }

    /// Preserve a real completed result unless its retained resource owner died.
    /// The single terminal uncertainty report repeats until explicit ACK.
    fn observe_disconnect(&self) {
        if !self.disconnected() {
            return;
        }
        let mut mailbox = self.mailbox.lock().expect("save mailbox poisoned");
        let pending = mailbox.pending.take();
        if mailbox.active.is_some() || mailbox.retained {
            if let Some(validity) = &mailbox.validity {
                validity.store(false, Ordering::Release);
            }
            let previous = mailbox.report.or(mailbox.previous);
            let (last_known_outcome, last_completed) = match previous.map(|report| report.outcome) {
                Some(SaveOutcome::Saved { receipt }) => (Some(receipt.known), Some(receipt)),
                Some(SaveOutcome::Failed(failure)) => {
                    (failure.current_outcome, failure.last_completed)
                }
                Some(SaveOutcome::Released {
                    current_outcome,
                    last_completed,
                }) => (current_outcome, last_completed),
                Some(SaveOutcome::Uncertain {
                    last_known_outcome,
                    last_completed,
                }) => (last_known_outcome, last_completed),
                None => (None, None),
            };
            let operation = mailbox.active.unwrap_or_else(|| {
                mailbox
                    .report
                    .expect("retained save owner has a terminal report")
                    .operation
            });
            mailbox.report = Some(SaveReport {
                id: mailbox.next,
                operation,
                outcome: SaveOutcome::Uncertain {
                    last_known_outcome,
                    last_completed,
                },
            });
            mailbox.active = None;
            mailbox.retained = false;
        }
        drop(mailbox);
        // Capabilities/large buffers are never reclaimed under the mailbox lock.
        drop(pending);
    }
}

/// One nonreplaceable owner of reviewed native saves and retained resources.
/// Host presentation generations cannot suppress an accepted terminal outcome.
/// Dropping the host does not join the actor or skip accepted publication.
pub struct SaveWorker {
    shared: Arc<Shared>,
    startup: Result<(), std::io::ErrorKind>,
}

impl SaveWorker {
    /// Whether this compiled host implements explicit database-options saves.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_database_options() -> bool {
        true
    }

    /// Whether this compiled host implements native query-expression saves.
    /// This is compile-target support, not actor health or source eligibility.
    pub const fn supports_query_expression() -> bool {
        true
    }

    /// Whether this host can consume native full-SQL review capabilities.
    /// This is compile-target support, not actor health or source eligibility.
    pub const fn supports_query_sql() -> bool {
        true
    }

    /// Compile-target support, independent of actor health and source eligibility.
    pub const fn supports_table_drop() -> bool {
        true
    }

    /// Consume one checked table DROP review through the retained save actor.
    ///
    /// # Errors
    /// Refusal returns the capability, following [`Self::start`].
    ///
    /// # Panics
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_table_drop(
        &self,
        reviewed: ReviewedTableDrop,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableDrop>> {
        // NO_CLONE: The actor and document must observe the same revocation latch.
        // Arc shares that atomic state across threads; copying it would detach validity.
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyTableDrop)
    }

    /// Whether this host can consume actual native saved-field DROP reviews.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_field_drop() -> bool {
        true
    }

    /// Whether this compiled host implements saved-field rename saves.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_field_rename() -> bool {
        true
    }

    /// Whether this compiled host implements the typed field-addition service.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_field_addition() -> bool {
        true
    }

    /// Whether this compiled host implements the explicit Number-column service.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_number_column() -> bool {
        true
    }

    /// Whether this compiled host implements the bounded no-default column service.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_add_column() -> bool {
        true
    }

    /// Whether this compiled host implements native table rename saves.
    /// This is compile-target support, not actor health or source eligibility.
    pub const fn supports_table_rename() -> bool {
        true
    }

    /// Whether this compiled host implements native table creation saves.
    /// This is compile-target support, not actor health or source eligibility.
    #[cfg(feature = "ace16-create")]
    pub const fn supports_table_creation() -> bool {
        true
    }

    /// Construct a nonreplaceable save owner without accepting native work.
    /// OS startup failure is retained for the next start attempt. A later actor death
    /// is terminal; a new source owner must be established explicitly.
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_spawner(repaint, ThreadSpawner)
    }

    fn with_spawner(repaint: impl Fn() + Send + Sync + 'static, spawner: impl Spawner) -> Self {
        let (signal, receiver) = task_signal::channel();
        let (alive, liveness) = sync_channel::<Infallible>(0);
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox {
                next: 0,
                active: None,
                validity: None,
                previous: None,
                retained: false,
                pending: None,
                report: None,
            }),
            liveness: Mutex::new(liveness),
            ready: signal,
            cancelled: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });
        let owner = Arc::clone(&shared);
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let startup = spawner
            .spawn(move || {
                // First local drops last: native owners and source validity end
                // before disconnection can be observed by application callers.
                let _exit = ActorExit {
                    alive: Some(alive),
                    repaint: Arc::clone(&repaint),
                };
                let mut source = ActorSource {
                    validity: None,
                    graceful: false,
                };
                let _subscriber = tracing::dispatcher::set_default(&dispatch);
                let mut retained: Option<RetainedOwner> = None;
                loop {
                    // A stop wake may have coalesced with the task's wake.
                    // Drain accepted work, then exit before blocking again.
                    if owner.stopped.load(Ordering::Acquire)
                        && owner
                            .mailbox
                            .lock()
                            .expect("save mailbox poisoned")
                            .pending
                            .is_none()
                    {
                        break;
                    }
                    if !receiver.wait() {
                        break;
                    }
                    let mut mailbox = owner.mailbox.lock().expect("save mailbox poisoned");
                    let Some((id, task)) = mailbox.pending.take() else {
                        if owner.stopped.load(Ordering::Acquire) {
                            break;
                        }
                        continue;
                    };
                    // NO_CLONE: The save actor guard retains the source validity latch after
                    // releasing the mailbox lock; uncertainty can invalidate the same
                    // source even when the mailbox owner still retains it.
                    source.validity = mailbox.validity.clone();
                    drop(mailbox);
                    let cancelled = || owner.cancelled.load(Ordering::Acquire);
                    let operation = match &task {
                        #[cfg(test)]
                        Task::Panic(_) => SaveOperation::Apply,
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyDatabaseOptions(..) => SaveOperation::Apply,
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyTableCreation(..)
                        | Task::ApplyIndexCreation(..)
                        | Task::ApplyIndexDrop(..) => SaveOperation::Apply,
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyAddColumn(..)
                        | Task::ApplyNumberColumn(..)
                        | Task::ApplyFieldAddition(..)
                        | Task::ApplyFieldRename(..)
                        | Task::ApplyFieldDrop(..) => SaveOperation::Apply,
                        Task::ApplyQueryExpression(..)
                        | Task::ApplyQuerySql(..)
                        | Task::Apply(..)
                        | Task::ApplyTableRename(..)
                        | Task::ApplyTableDrop(..)
                        | Task::ApplyInsert(..)
                        | Task::ApplyDelete(..)
                        | Task::ApplyQueryInsert(..)
                        | Task::ApplyQueryDelete(..)
                        | Task::ApplyQueryUpdate(..) => SaveOperation::Apply,
                        Task::Restart(..) | Task::Recover => SaveOperation::Recover,
                        Task::Release => SaveOperation::Release,
                    };
                    let operation_name = match operation {
                        SaveOperation::Apply => "apply",
                        SaveOperation::Recover => "recover",
                        SaveOperation::Release => "release",
                    };
                    let span =
                        tracing::debug_span!("application save task", operation = operation_name);
                    let _entered = span.enter();
                    let outcome = match task {
                        #[cfg(test)]
                        Task::Panic(_task) => panic!("injected native actor failure"),
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyDatabaseOptions(reviewed, bundle) => {
                            apply_database_options(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyQueryExpression(reviewed, bundle) => {
                            apply_query_expression(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyTableDrop(reviewed, bundle) => {
                            self::apply_table_drop::apply_table_drop(
                                reviewed,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        Task::ApplyQuerySql(reviewed, bundle) => {
                            apply_query_sql(reviewed, bundle, &mut retained, &cancelled)
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyFieldDrop(reviewed, bundle) => {
                            self::apply_field_drop::apply_field_drop(
                                reviewed,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyFieldRename(reviewed, bundle) => {
                            self::apply_field_rename::apply_field_rename(
                                reviewed,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyFieldAddition(reviewed, bundle) => {
                            self::apply_field_addition::apply_field_addition(
                                reviewed,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyNumberColumn(reviewed, bundle) => {
                            self::apply_field_addition::apply_number_column(
                                reviewed,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyAddColumn(reviewed, bundle) => {
                            apply_add_column(reviewed, bundle, &mut retained, &cancelled)
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyIndexCreation(reviewed, bundle) => {
                            apply_index_creation(reviewed, bundle, &mut retained, &cancelled)
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyIndexDrop(reviewed, bundle) => {
                            apply_index_drop(reviewed, bundle, &mut retained, &cancelled)
                        }
                        #[cfg(feature = "ace16-create")]
                        Task::ApplyTableCreation(reviewed, bundle) => {
                            apply_table_creation(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyTableRename(reviewed, bundle) => {
                            apply_table_rename(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::Apply(reviewed, bundle) => {
                            apply(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyInsert(reviewed, bundle) => {
                            apply_insert(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyDelete(reviewed, bundle) => {
                            apply_delete(reviewed, bundle, &mut retained, &cancelled)
                        }
                        Task::ApplyQueryUpdate(reviewed, bundle) => {
                            let reviewed = *reviewed.state;
                            apply_query_action(
                                reviewed.target,
                                Plan::Update(reviewed.plan),
                                reviewed.valid,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        Task::ApplyQueryDelete(reviewed, bundle) => {
                            let reviewed = *reviewed.state;
                            apply_query_action(
                                reviewed.target,
                                Plan::Delete(reviewed.plan),
                                reviewed.valid,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        Task::ApplyQueryInsert(reviewed, bundle) => {
                            let reviewed = *reviewed.state;
                            apply_query_action(
                                reviewed.target,
                                Plan::Insert(reviewed.plan),
                                reviewed.valid,
                                bundle,
                                &mut retained,
                                &cancelled,
                            )
                        }
                        Task::Restart(source, bundle) => {
                            restart(source, bundle, &mut retained, &cancelled)
                        }
                        Task::Recover => match retained.take().map(RetainedOwner::into_session) {
                            Some(session) => {
                                // Recovery includes complete journal image validation and
                                // fresh native reconstruction, separately from the normal
                                // native planning allowance used before an initial write.
                                let mut work = action_recovery_budget();
                                finish(
                                    session.recover_pending(&mut work, &cancelled),
                                    &mut retained,
                                )
                            }
                            None => SaveOutcome::Failed(no_session(SaveFailureKind::InvalidState)),
                        },
                        Task::Release => match retained.take().map(RetainedOwner::into_session) {
                            Some(session) => {
                                let current_outcome = session.current_outcome().map(known);
                                let last_completed = session.last_completed().map(receipt);
                                match session.release() {
                                    Ok(()) => SaveOutcome::Released {
                                        current_outcome,
                                        last_completed,
                                    },
                                    Err(failure) => retain(failure, &mut retained),
                                }
                            }
                            None => SaveOutcome::Failed(no_session(SaveFailureKind::InvalidState)),
                        },
                    };
                    tracing::debug!(
                        retained_resources = retained.is_some(),
                        "application save task completed"
                    );
                    let mut mailbox = owner.mailbox.lock().expect("save mailbox poisoned");
                    mailbox.active = None;
                    mailbox.retained = retained.is_some();
                    mailbox.report = Some(SaveReport {
                        id,
                        operation,
                        outcome,
                    });
                    drop(mailbox);
                    if catch_unwind(AssertUnwindSafe(|| repaint())).is_err() {
                        tracing::error!("save completion callback panicked");
                    }
                }
                source.graceful = owner.stopped.load(Ordering::Acquire);
            })
            .map_err(|error| error.kind());
        Self { shared, startup }
    }

    /// Consume only the explicitly reviewed option request through this save owner.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_disable_name_auto_correct(
        &self,
        reviewed: ReviewedDatabaseOptions,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedDatabaseOptions>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyDatabaseOptions)
    }

    /// Consume one document-bound cell review through this retained save owner.
    /// Acceptance returns a worker-local identity; publication and cleanup outcomes
    /// arrive later through [`Self::poll`].
    ///
    /// # Errors
    ///
    /// Returns [`SaveStartFailure::ThreadStart`] if the OS refused actor startup,
    /// [`SaveStartFailure::WorkerStopped`] if that actor ended,
    /// [`SaveStartFailure::StaleDocument`] for an invalidated review,
    /// [`SaveStartFailure::Busy`] while another task/report/native owner is retained,
    /// or [`SaveStartFailure::Exhausted`] when no new identity can be issued.
    /// Each refusal returns this request's original unconsumed capability, without
    /// filesystem work for the new request. It says nothing about earlier writes.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start(&self, reviewed: ReviewedEdit, bundle: PathBuf) -> Result<u64, SaveStartError> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::Apply)
    }

    /// Consume an exact reviewed row insertion without reevaluating its inputs.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_insert(
        &self,
        reviewed: ReviewedInsert,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedInsert>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyInsert)
    }

    /// Consume an exact reviewed row deletion through the retained save owner.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_delete(
        &self,
        reviewed: ReviewedDelete,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedDelete>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyDelete)
    }

    /// Consume an unlocked SQL UPDATE review through the same nonreplaceable
    /// save owner. Rejection returns the original review without filesystem work.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_query_update(
        &self,
        reviewed: ReviewedQueryUpdate,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryUpdate>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQueryUpdate)
    }

    /// Consume one unlocked SQL DELETE review through the same retained save owner.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_query_delete(
        &self,
        reviewed: ReviewedQueryDelete,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryDelete>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQueryDelete)
    }

    /// Consume one unlocked SQL INSERT review without reevaluating its inputs.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_query_insert(
        &self,
        reviewed: ReviewedQueryInsert,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryInsert>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQueryInsert)
    }

    /// Consume the exact full-SQL review, without reparsing or executing SQL.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_query_sql(
        &self,
        reviewed: ReviewedQuerySql,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQuerySql>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQuerySql)
    }

    /// Consume one document-owned existing-query review without SQL replay.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_query_expression(
        &self,
        reviewed: ReviewedQueryExpression,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryExpression>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQueryExpression)
    }

    /// Consume one exact DROP review; absent targets never become no-op saves.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_field_drop(
        &self,
        reviewed: ReviewedFieldDrop,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedFieldDrop>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyFieldDrop)
    }

    /// Consume the exact original-field rename review once.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior otherwise match [`Self::start`].
    /// An unchanged review returns [`SaveStartFailure::InvalidState`] without a save
    /// identity or journal. Its consuming readonly witness is available through
    /// [`ReviewedFieldRename::into_unchanged_reopen`] instead.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_field_rename(
        &self,
        reviewed: ReviewedFieldRename,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedFieldRename>> {
        if !reviewed.changed() {
            return Err(SaveStartError {
                kind: SaveStartFailure::InvalidState,
                reviewed: Some(reviewed),
            });
        }
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyFieldRename)
    }

    /// Consume one exact typed field-addition review.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_field_addition(
        &self,
        reviewed: ReviewedFieldAddition,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedFieldAddition>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyFieldAddition)
    }

    /// Consume one exact review from the explicit Number service.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_number_column(
        &self,
        reviewed: ReviewedNumberColumn,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedNumberColumn>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyNumberColumn)
    }

    /// Consume one exact review from the bounded no-default column service.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_add_column(
        &self,
        reviewed: ReviewedAddColumn,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedAddColumn>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyAddColumn)
    }

    /// Consume one exact native table-creation review.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    #[cfg(feature = "ace16-create")]
    pub fn start_table_creation(
        &self,
        reviewed: ReviewedTableCreation,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableCreation>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyTableCreation)
    }

    /// Consume one source-issued original index request on the retained save actor.
    /// Refusal returns the capability; no file lease exists while awaiting review.
    #[cfg(feature = "ace16-create")]
    pub fn start_index_creation(
        &self,
        reviewed: ReviewedIndexCreation,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedIndexCreation>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyIndexCreation)
    }

    /// Consume one original index DROP; refusal returns the same capability.
    #[cfg(feature = "ace16-create")]
    pub fn start_index_drop(
        &self,
        reviewed: ReviewedIndexDrop,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedIndexDrop>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyIndexDrop)
    }

    /// Consume the exact lease-free rename reviewed by the current document.
    ///
    /// # Errors
    ///
    /// Refusal and returned-capability behavior match [`Self::start`].
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_table_rename(
        &self,
        reviewed: ReviewedTableRename,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableRename>> {
        let valid = Some(Arc::clone(&reviewed.state.valid));
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyTableRename)
    }

    /// Explicitly attempt checked recovery of this source's reserved sibling.
    /// No target is opened before acceptance. Rejection returns the source path;
    /// decoding, native qualification and all writes stay with the shared backend.
    ///
    /// # Errors
    ///
    /// Returns [`SaveStartFailure::ThreadStart`], [`SaveStartFailure::WorkerStopped`],
    /// [`SaveStartFailure::Busy`] or [`SaveStartFailure::Exhausted`] with the original
    /// source path. Refusal performs no filesystem work for this request; it does not
    /// classify the source or establish an earlier operation's outcome.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn start_reserved_recovery(&self, source: PathBuf) -> Result<u64, SaveStartError<PathBuf>> {
        let bundle = crate::editing::journal_path(&source);
        self.start_reviewed_operation(source, bundle, None, Task::Restart, SaveOperation::Recover)
    }

    fn start_reviewed<T>(
        &self,
        reviewed: T,
        bundle: PathBuf,
        valid: Option<Arc<AtomicBool>>,
        task: fn(T, PathBuf) -> Task,
    ) -> Result<u64, SaveStartError<T>> {
        self.start_reviewed_operation(reviewed, bundle, valid, task, SaveOperation::Apply)
    }

    fn start_reviewed_operation<T>(
        &self,
        reviewed: T,
        bundle: PathBuf,
        valid: Option<Arc<AtomicBool>>,
        task: fn(T, PathBuf) -> Task,
        operation: SaveOperation,
    ) -> Result<u64, SaveStartError<T>> {
        if let Err(kind) = self.startup {
            return Err(SaveStartError {
                kind: SaveStartFailure::ThreadStart(kind),
                reviewed: Some(reviewed),
            });
        }
        if self.shared.disconnected() {
            self.shared.observe_disconnect();
            return Err(SaveStartError {
                kind: SaveStartFailure::WorkerStopped,
                reviewed: Some(reviewed),
            });
        }
        if valid
            .as_ref()
            .is_some_and(|valid| !valid.load(Ordering::Acquire))
        {
            return Err(SaveStartError {
                kind: SaveStartFailure::StaleDocument,
                reviewed: Some(reviewed),
            });
        }
        let mut mailbox = self.shared.mailbox.lock().expect("save mailbox poisoned");
        if mailbox.active.is_some() || mailbox.report.is_some() || mailbox.retained {
            return Err(SaveStartError {
                kind: SaveStartFailure::Busy,
                reviewed: Some(reviewed),
            });
        }
        let Some(id) = mailbox.next.checked_add(1) else {
            return Err(SaveStartError {
                kind: SaveStartFailure::Exhausted,
                reviewed: Some(reviewed),
            });
        };
        mailbox.next = id;
        mailbox.active = Some(operation);
        mailbox.validity = valid;
        mailbox.previous = None;
        mailbox.pending = Some((id, task(reviewed, bundle)));
        self.shared.cancelled.store(false, Ordering::Release);
        drop(mailbox);
        self.shared.ready.notify();
        Ok(id)
    }

    /// Repeated polling observes the SAME terminal report until explicit acknowledgement.
    /// If an accepted operation or retained resource owner is lost when the actor
    /// ends, the terminal report is [`SaveOutcome::Uncertain`]. Its optional facts
    /// are historical only, not proof of the current file or permission to retry.
    /// An already completed resource-free report is preserved unchanged.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn poll(&self) -> Option<SaveReport> {
        self.shared.observe_disconnect();
        self.shared
            .mailbox
            .lock()
            .expect("save mailbox poisoned")
            .report
    }

    /// Whether an accepted task, unacknowledged report or retained native owner
    /// blocks a new operation. Startup failure is observed through start refusal,
    /// not inferred from this busy flag.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn is_busy(&self) -> bool {
        self.shared.observe_disconnect();
        let mailbox = self.shared.mailbox.lock().expect("save mailbox poisoned");
        mailbox.active.is_some() || mailbox.report.is_some() || mailbox.retained
    }

    /// Request cancellation of the exact active identity without asserting that
    /// native publication stopped. Its terminal report remains required.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn cancel(&self, id: u64) -> bool {
        let mailbox = self.shared.mailbox.lock().expect("save mailbox poisoned");
        if mailbox.next != id || mailbox.active.is_none() {
            return false;
        }
        self.shared.cancelled.store(true, Ordering::Release);
        true
    }

    /// Dismiss only the matching terminal report when no native owner is retained.
    /// Acknowledging uncertainty never verifies the file, cleans a journal or makes
    /// the ended actor reusable.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn acknowledge(&self, id: u64) -> bool {
        self.shared.observe_disconnect();
        let mut mailbox = self.shared.mailbox.lock().expect("save mailbox poisoned");
        if mailbox.active.is_some()
            || mailbox.retained
            || !mailbox.report.is_some_and(|report| report.id == id)
        {
            return false;
        }
        mailbox.report = None;
        mailbox.previous = None;
        mailbox.validity = None;
        true
    }

    /// Request checked recovery through the exact retained session/report identity.
    /// The operation runs on the actor only after acceptance; no reviewed capability
    /// is supplied by this call.
    ///
    /// # Errors
    ///
    /// Returns [`SaveStartFailure::ThreadStart`] or [`SaveStartFailure::WorkerStopped`]
    /// if no actor can run the request, [`SaveStartFailure::InvalidState`] unless the
    /// matching terminal report retains an owner with the requested permission, or
    /// [`SaveStartFailure::Exhausted`] when identities are exhausted.
    /// [`SaveStartError::into_reviewed`] returns `None` for these report-only requests.
    /// Refusal grants no cleanup, rollback or retry authority.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn recover(&self, id: u64) -> Result<u64, SaveStartError> {
        self.continue_retained(id, true)
    }

    /// Request explicit release through the exact retained session/report identity.
    /// The operation runs on the actor only after acceptance; no reviewed capability
    /// is supplied by this call.
    ///
    /// # Errors
    ///
    /// Returns [`SaveStartFailure::ThreadStart`] or [`SaveStartFailure::WorkerStopped`]
    /// if no actor can run the request, [`SaveStartFailure::InvalidState`] unless the
    /// matching terminal report retains an owner with the requested permission, or
    /// [`SaveStartFailure::Exhausted`] when identities are exhausted.
    /// [`SaveStartError::into_reviewed`] returns `None` for these report-only requests.
    /// Refusal grants no cleanup, rollback or retry authority.
    ///
    /// # Panics
    ///
    /// Panics when a required internal synchronization lock is poisoned.
    pub fn release(&self, id: u64) -> Result<u64, SaveStartError> {
        self.continue_retained(id, false)
    }

    fn continue_retained(&self, id: u64, recover: bool) -> Result<u64, SaveStartError> {
        if let Err(kind) = self.startup {
            return Err(SaveStartError {
                kind: SaveStartFailure::ThreadStart(kind),
                reviewed: None,
            });
        }
        if self.shared.disconnected() {
            self.shared.observe_disconnect();
            return Err(SaveStartError {
                kind: SaveStartFailure::WorkerStopped,
                reviewed: None,
            });
        }
        let mut mailbox = self.shared.mailbox.lock().expect("save mailbox poisoned");
        let allowed = mailbox.report.is_some_and(|report| {
            report.id == id
                && match report.outcome {
                    SaveOutcome::Failed(failure) => {
                        if recover {
                            failure.can_recover
                        } else {
                            failure.can_release
                        }
                    }
                    _ => false,
                }
        });
        if mailbox.active.is_some() || !mailbox.retained || !allowed {
            return Err(SaveStartError {
                kind: SaveStartFailure::InvalidState,
                reviewed: None,
            });
        }
        let next = mailbox.next.checked_add(1).ok_or(SaveStartError {
            kind: SaveStartFailure::Exhausted,
            reviewed: None,
        })?;
        mailbox.next = next;
        mailbox.active = Some(if recover {
            SaveOperation::Recover
        } else {
            SaveOperation::Release
        });
        mailbox.previous = mailbox.report.take();
        mailbox.pending = Some((
            next,
            if recover {
                Task::Recover
            } else {
                Task::Release
            },
        ));
        self.shared.cancelled.store(false, Ordering::Release);
        drop(mailbox);
        self.shared.ready.notify();
        Ok(next)
    }
}

impl Drop for SaveWorker {
    fn drop(&mut self) {
        // Never interrupt publication or auto-clean retained bundles. The host
        // must resolve busy/retained state before allowing application exit.
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.ready.notify();
    }
}

#[cfg(test)]
mod startup_tests;

#[cfg(test)]
mod health_tests;

#[cfg(test)]
mod test_support;
