//! The actual offline owner lives on this worker until released or diagnosed.
mod apply;
mod apply_delete;
mod apply_insert;
mod apply_query_update;
#[cfg(feature = "ace16-create")]
mod apply_table_creation;
mod outcomes;
mod restart;

#[cfg(feature = "ace16-create")]
use self::apply_table_creation::apply_table_creation;
use self::{
    apply::apply,
    apply_delete::apply_delete,
    apply_insert::apply_insert,
    apply_query_update::apply_query_update,
    outcomes::{finish, known, no_session, receipt, retain},
    restart::restart,
};
use super::{
    SaveFailureKind, SaveOperation, SaveOutcome, SaveReport, SaveStartError, SaveStartFailure,
};
#[cfg(feature = "ace16-create")]
use crate::editing::ReviewedTableCreation;
use crate::{
    controls::update_recovery_budget,
    editing::{ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryUpdate},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_access::editing::offline::{Failure, OfflineWriteSession};

enum Task {
    #[cfg(feature = "ace16-create")]
    ApplyTableCreation(ReviewedTableCreation, PathBuf),
    Apply(ReviewedEdit, PathBuf),
    ApplyInsert(ReviewedInsert, PathBuf),
    ApplyDelete(ReviewedDelete, PathBuf),
    ApplyQueryUpdate(ReviewedQueryUpdate, PathBuf),
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
    active: bool,
    retained: bool,
    pending: Option<(u64, Task)>,
    report: Option<SaveReport>,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    ready: Condvar,
    cancelled: AtomicBool,
    stopped: AtomicBool,
}

pub struct SaveWorker {
    shared: Arc<Shared>,
}

impl SaveWorker {
    #[cfg(feature = "ace16-create")]
    pub const fn supports_table_creation() -> bool {
        true
    }

    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox {
                next: 0,
                active: false,
                retained: false,
                pending: None,
                report: None,
            }),
            ready: Condvar::new(),
            cancelled: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });
        let owner = Arc::clone(&shared);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::spawn(move || {
            let _subscriber = tracing::dispatcher::set_default(&dispatch);
            let mut retained: Option<RetainedOwner> = None;
            loop {
                let mut mailbox = owner.mailbox.lock().unwrap();
                while mailbox.pending.is_none() && !owner.stopped.load(Ordering::Acquire) {
                    mailbox = owner.ready.wait(mailbox).unwrap();
                }
                if mailbox.pending.is_none() && owner.stopped.load(Ordering::Acquire) {
                    break;
                }
                let Some((id, task)) = mailbox.pending.take() else {
                    continue;
                };
                drop(mailbox);
                let cancelled = || owner.cancelled.load(Ordering::Acquire);
                let operation = match &task {
                    #[cfg(feature = "ace16-create")]
                    Task::ApplyTableCreation(..) => SaveOperation::Apply,
                    Task::Apply(..)
                    | Task::ApplyInsert(..)
                    | Task::ApplyDelete(..)
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
                    #[cfg(feature = "ace16-create")]
                    Task::ApplyTableCreation(reviewed, bundle) => {
                        apply_table_creation(reviewed, bundle, &mut retained, &cancelled)
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
                        apply_query_update(reviewed, bundle, &mut retained, &cancelled)
                    }
                    Task::Restart(source, bundle) => {
                        restart(source, bundle, &mut retained, &cancelled)
                    }
                    Task::Recover => match retained.take().map(RetainedOwner::into_session) {
                        Some(session) => {
                            // Recovery includes complete journal image validation and
                            // fresh native reconstruction, separately from the normal
                            // native planning allowance used before an initial write.
                            let mut work = update_recovery_budget();
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
                let mut mailbox = owner.mailbox.lock().unwrap();
                mailbox.active = false;
                mailbox.retained = retained.is_some();
                mailbox.report = Some(SaveReport {
                    id,
                    operation,
                    outcome,
                });
                drop(mailbox);
                if catch_unwind(AssertUnwindSafe(&repaint)).is_err() {
                    tracing::error!("save completion callback panicked");
                }
            }
        });
        Self { shared }
    }

    pub fn start(&self, reviewed: ReviewedEdit, bundle: PathBuf) -> Result<u64, SaveStartError> {
        let valid = reviewed.state.valid.load(Ordering::Acquire);
        self.start_reviewed(reviewed, bundle, valid, Task::Apply)
    }

    pub fn start_insert(
        &self,
        reviewed: ReviewedInsert,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedInsert>> {
        let valid = reviewed.state.valid.load(Ordering::Acquire);
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyInsert)
    }

    pub fn start_delete(
        &self,
        reviewed: ReviewedDelete,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedDelete>> {
        let valid = reviewed.state.valid.load(Ordering::Acquire);
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyDelete)
    }

    /// Consume an unlocked SQL UPDATE review through the same nonreplaceable
    /// save owner. Rejection returns the original review without filesystem work.
    pub fn start_query_update(
        &self,
        reviewed: ReviewedQueryUpdate,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedQueryUpdate>> {
        let valid = reviewed.state.valid.load(Ordering::Acquire);
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyQueryUpdate)
    }

    #[cfg(feature = "ace16-create")]
    pub fn start_table_creation(
        &self,
        reviewed: ReviewedTableCreation,
        bundle: PathBuf,
    ) -> Result<u64, SaveStartError<ReviewedTableCreation>> {
        let valid = reviewed.state.valid.load(Ordering::Acquire);
        self.start_reviewed(reviewed, bundle, valid, Task::ApplyTableCreation)
    }

    /// Explicitly attempt checked recovery of this source's reserved sibling.
    /// No target is opened before acceptance. Rejection returns the source path;
    /// decoding, native qualification and all writes stay with the shared backend.
    pub fn start_reserved_recovery(&self, source: PathBuf) -> Result<u64, SaveStartError<PathBuf>> {
        let bundle = crate::editing::journal_path(&source);
        self.start_reviewed(source, bundle, true, Task::Restart)
    }

    fn start_reviewed<T>(
        &self,
        reviewed: T,
        bundle: PathBuf,
        valid: bool,
        task: fn(T, PathBuf) -> Task,
    ) -> Result<u64, SaveStartError<T>> {
        if !valid {
            return Err(SaveStartError {
                kind: SaveStartFailure::StaleDocument,
                reviewed: Some(reviewed),
            });
        }
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.active || mailbox.report.is_some() || mailbox.retained {
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
        mailbox.active = true;
        mailbox.pending = Some((id, task(reviewed, bundle)));
        self.shared.cancelled.store(false, Ordering::Release);
        drop(mailbox);
        self.shared.ready.notify_one();
        Ok(id)
    }

    /// Repeated polling observes the SAME terminal report until explicit acknowledgement.
    pub fn poll(&self) -> Option<SaveReport> {
        self.shared.mailbox.lock().unwrap().report
    }

    pub fn is_busy(&self) -> bool {
        let mailbox = self.shared.mailbox.lock().unwrap();
        mailbox.active || mailbox.report.is_some() || mailbox.retained
    }

    pub fn cancel(&self, id: u64) -> bool {
        let mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.next != id || !mailbox.active {
            return false;
        }
        self.shared.cancelled.store(true, Ordering::Release);
        true
    }

    pub fn acknowledge(&self, id: u64) -> bool {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
        if mailbox.active
            || mailbox.retained
            || !mailbox.report.is_some_and(|report| report.id == id)
        {
            return false;
        }
        mailbox.report = None;
        true
    }

    pub fn recover(&self, id: u64) -> Result<u64, SaveStartError> {
        self.continue_retained(id, true)
    }
    pub fn release(&self, id: u64) -> Result<u64, SaveStartError> {
        self.continue_retained(id, false)
    }

    fn continue_retained(&self, id: u64, recover: bool) -> Result<u64, SaveStartError> {
        let mut mailbox = self.shared.mailbox.lock().unwrap();
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
        if mailbox.active || !mailbox.retained || !allowed {
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
        mailbox.active = true;
        mailbox.report = None;
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
        self.shared.ready.notify_one();
        Ok(next)
    }
}

impl Drop for SaveWorker {
    fn drop(&mut self) {
        // Never interrupt publication or auto-clean retained bundles. The host
        // must resolve busy/retained state before allowing application exit.
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.ready.notify_one();
    }
}
