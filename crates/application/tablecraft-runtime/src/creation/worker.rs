//! A fresh creation and its terminal receipt cannot be superseded by read views.
use super::{
    CreationFailure, CreationOutcome, CreationReport, CreationRequest, CreationStartError,
    CreationStartFailure, service,
};
use crate::{
    task_signal,
    thread_start::{Spawner, ThreadSpawner},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Mailbox {
    next: u64,
    active: bool,
    pending: Option<Arc<CreationReport>>,
    report: Option<Arc<CreationReport>>,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    ready: task_signal::Signal,
    cancelled: AtomicBool,
    stopped: AtomicBool,
}

/// Hosts retain this owner until its explicit terminal acknowledgement.
/// Cancellation requests do not assert absence of publication or remove a file.
pub struct CreationWorker {
    shared: Arc<Shared>,
    startup: Result<(), std::io::ErrorKind>,
}

impl CreationWorker {
    pub const fn is_supported() -> bool {
        cfg!(any(unix, windows))
    }

    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_operation(repaint, service::blank)
    }

    fn with_operation(
        repaint: impl Fn() + Send + Sync + 'static,
        operation: impl Fn(&Path, &dyn Fn() -> bool) -> CreationOutcome + Send + 'static,
    ) -> Self {
        Self::with_operation_and_spawner(repaint, operation, ThreadSpawner)
    }

    fn with_operation_and_spawner(
        repaint: impl Fn() + Send + Sync + 'static,
        operation: impl Fn(&Path, &dyn Fn() -> bool) -> CreationOutcome + Send + 'static,
        spawner: impl Spawner,
    ) -> Self {
        let (signal, receiver) = task_signal::channel();
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox {
                next: 0,
                active: false,
                pending: None,
                report: None,
            }),
            ready: signal,
            cancelled: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        });

        let startup = if Self::is_supported() {
            let owner = Arc::clone(&shared);
            let dispatch = tracing::dispatcher::get_default(Clone::clone);
            spawner
                .spawn(move || {
                    let _subscriber = tracing::dispatcher::set_default(&dispatch);
                    loop {
                        // A stop wake may have coalesced with the task's wake.
                        // Drain accepted work, then exit before blocking again.
                        if owner.stopped.load(Ordering::Acquire)
                            && owner
                                .mailbox
                                .lock()
                                .expect("creation mailbox poisoned")
                                .pending
                                .is_none()
                        {
                            break;
                        }
                        if !receiver.wait() {
                            break;
                        }
                        let mut mailbox = owner.mailbox.lock().expect("creation mailbox poisoned");
                        let Some(mut report) = mailbox.pending.take() else {
                            if owner.stopped.load(Ordering::Acquire) {
                                break;
                            }
                            continue;
                        };
                        drop(mailbox);

                        let _span = tracing::debug_span!(
                            "application creation task",
                            operation = "blank_ace16"
                        )
                        .entered();
                        let cancelled = || owner.cancelled.load(Ordering::Acquire);
                        let outcome = catch_unwind(AssertUnwindSafe(|| {
                            operation(report.destination(), &cancelled)
                        }))
                        .unwrap_or_else(|_| CreationOutcome::Failed(CreationFailure::panic()));
                        let created = matches!(outcome, CreationOutcome::Created(_));

                        // The report/path/Arc allocation precedes all filesystem work.
                        // No observer can see it before this one terminal assignment.
                        Arc::get_mut(&mut report)
                            .expect("only the operation owns its unpublished report")
                            .outcome = Some(outcome);
                        let mut mailbox = owner.mailbox.lock().expect("creation mailbox poisoned");
                        mailbox.active = false;
                        mailbox.report = Some(report);
                        drop(mailbox);

                        tracing::debug!(created, "application creation task completed");
                        if catch_unwind(AssertUnwindSafe(&repaint)).is_err() {
                            tracing::error!("creation completion callback panicked");
                        }
                    }
                })
                .map_err(|error| error.kind())
        } else {
            Ok(())
        };
        Self { shared, startup }
    }

    pub fn start(&self, request: CreationRequest) -> Result<u64, CreationStartError> {
        if !Self::is_supported() {
            return Err(CreationStartError {
                kind: CreationStartFailure::UnsupportedPlatform,
                request,
            });
        }
        if let Err(kind) = self.startup {
            return Err(CreationStartError {
                kind: CreationStartFailure::ThreadStart(kind),
                request,
            });
        }
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .expect("creation mailbox poisoned");
        if mailbox.active || mailbox.report.is_some() {
            return Err(CreationStartError {
                kind: CreationStartFailure::Busy,
                request,
            });
        }
        let Some(id) = mailbox.next.checked_add(1) else {
            return Err(CreationStartError {
                kind: CreationStartFailure::Exhausted,
                request,
            });
        };

        let report = Arc::new(CreationReport {
            id,
            operation: request.operation(),
            destination: request.destination,
            outcome: None,
        });
        mailbox.next = id;
        mailbox.active = true;
        mailbox.pending = Some(report);
        self.shared.cancelled.store(false, Ordering::Release);
        drop(mailbox);
        self.shared.ready.notify();
        Ok(id)
    }

    /// Repeated polling clones the same preallocated terminal receipt owner.
    pub fn poll(&self) -> Option<Arc<CreationReport>> {
        self.shared
            .mailbox
            .lock()
            .expect("creation mailbox poisoned")
            .report
            .clone()
    }

    pub fn cancel(&self, id: u64) -> bool {
        let mailbox = self
            .shared
            .mailbox
            .lock()
            .expect("creation mailbox poisoned");
        if mailbox.next != id || !mailbox.active {
            return false;
        }
        self.shared.cancelled.store(true, Ordering::Release);
        true
    }

    pub fn acknowledge(&self, id: u64) -> bool {
        let mut mailbox = self
            .shared
            .mailbox
            .lock()
            .expect("creation mailbox poisoned");
        if mailbox.active
            || !mailbox
                .report
                .as_ref()
                .is_some_and(|report| report.id == id)
        {
            return false;
        }
        mailbox.report = None;
        true
    }

    pub fn is_busy(&self) -> bool {
        let mailbox = self
            .shared
            .mailbox
            .lock()
            .expect("creation mailbox poisoned");
        mailbox.active || mailbox.report.is_some()
    }
}

impl Drop for CreationWorker {
    fn drop(&mut self) {
        // Dropping a host never cancels publication or removes a known output.
        // The host must keep this owner until its terminal result is acknowledged.
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.ready.notify();
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, any(unix, windows)))]
mod startup_tests;
