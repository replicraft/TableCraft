use parking_lot::Mutex;
use semver::Version;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender, TryRecvError, TrySendError},
    },
    thread,
};
use tablecraft_update::{
    UpdateError,
    apply::{AppliedUpdate, ApplyError},
    platform::Platform,
    session::{Session, UpdateCommand, UpdateEvent, UpdateTaskError},
};

const EVENT_CAPACITY: usize = 8;

struct Task {
    generation: u64,
    command: UpdateCommand,
}

/// Native outcomes retain resource ownership independently of presentation delivery.
pub(crate) enum RetainedOutcome {
    RestartScheduled(AppliedUpdate),
    ApplyFailed(ApplyError),
}

pub(crate) enum Reply {
    Event {
        generation: u64,
        event: UpdateEvent,
    },
    Failed {
        generation: u64,
        error: UpdateTaskError,
    },
    Retained {
        generation: u64,
        outcome: Arc<RetainedOutcome>,
    },
    Stopped {
        generation: u64,
        installation_uncertain: bool,
    },
}

struct RetainedEntry<T> {
    generation: u64,
    outcome: Arc<T>,
}

struct Mailbox<T> {
    pending: Mutex<VecDeque<RetainedEntry<T>>>,
}

enum MailboxPoll<T> {
    Contended,
    Empty,
    Ready(RetainedEntry<T>),
}

impl<T> Mailbox<T> {
    fn new() -> Self {
        Self {
            pending: Mutex::new(VecDeque::with_capacity(1)),
        }
    }

    fn push(&self, entry: RetainedEntry<T>) {
        // The actor emits at most one native outcome per explicit command.
        // No host callback or filesystem operation runs under this short lock.
        self.pending.lock().push_back(entry);
    }

    fn poll(&self) -> MailboxPoll<T> {
        let Some(mut pending) = self.pending.try_lock() else {
            return MailboxPoll::Contended;
        };
        match pending.pop_front() {
            Some(entry) => MailboxPoll::Ready(entry),
            None => MailboxPoll::Empty,
        }
    }
}

pub(crate) struct UpdateWorker {
    commands: Option<Sender<Task>>,
    events: Receiver<Reply>,
    event_sender: Option<SyncSender<Reply>>,
    retained: Arc<Mailbox<RetainedOutcome>>,
    cancelled: Arc<AtomicBool>,
    repaint: Option<Box<dyn Fn() + Send>>,
    generation: u64,
    busy: bool,
    installing: bool,
    stopped: bool,
    native_terminal_received: bool,
}

impl UpdateWorker {
    pub(crate) fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let (event_sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        Self {
            commands: None,
            events,
            event_sender: Some(event_sender),
            retained: Arc::new(Mailbox::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
            repaint: Some(Box::new(repaint)),
            generation: 0,
            busy: false,
            installing: false,
            stopped: false,
            native_terminal_received: false,
        }
    }

    pub(crate) fn start(&mut self, command: UpdateCommand) -> bool {
        if self.busy || self.stopped {
            return false;
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return false;
        };
        if self.commands.is_none() && !self.spawn() {
            return false;
        }
        let installing = matches!(command, UpdateCommand::Install);
        self.cancelled.store(false, Ordering::Release);
        let task = Task {
            generation,
            command,
        };
        if !self
            .commands
            .as_ref()
            .is_some_and(|sender| sender.send(task).is_ok())
        {
            return false;
        }
        self.generation = generation;
        self.busy = true;
        self.installing = installing;
        self.native_terminal_received = false;
        true
    }

    pub(crate) fn cancel(&self) {
        if !self.installing {
            self.cancelled.store(true, Ordering::Release);
        }
    }

    pub(crate) fn installing(&self) -> bool {
        self.installing
    }

    pub(crate) fn poll(&mut self) -> Option<Reply> {
        loop {
            match self.retained.poll() {
                MailboxPoll::Ready(entry) => return Some(self.accept_retained(entry)),
                // The producer requests repaint only after releasing the lock.
                MailboxPoll::Contended => return None,
                MailboxPoll::Empty => {}
            }
            let reply = match self.events.try_recv() {
                Ok(reply) => reply,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    // A producer may have published its native terminal between
                    // the first mailbox probe and channel teardown. Check again.
                    return match self.retained.poll() {
                        MailboxPoll::Ready(entry) => Some(self.accept_retained(entry)),
                        MailboxPoll::Contended => None,
                        MailboxPoll::Empty => self.worker_stopped(),
                    };
                }
            };
            // Retain resource-bearing replies even from a late or legacy queue
            // route. Only replaceable presentation observations are generation-filtered.
            let reply = match split_retained(reply) {
                Ok(entry) => return Some(self.accept_retained(entry)),
                Err(reply) => reply,
            };
            let (generation, terminal) = match &reply {
                Reply::Event { generation, event } => (
                    *generation,
                    !matches!(
                        event,
                        UpdateEvent::Checking
                            | UpdateEvent::Downloading(_)
                            | UpdateEvent::Installing
                    ),
                ),
                Reply::Failed { generation, .. } | Reply::Stopped { generation, .. } => {
                    (*generation, true)
                }
                Reply::Retained { generation, .. } => (*generation, true),
            };
            if generation != self.generation {
                continue;
            }
            if terminal {
                self.busy = false;
                self.installing = false;
            }
            return Some(reply);
        }
    }

    fn accept_retained(&mut self, entry: RetainedEntry<RetainedOutcome>) -> Reply {
        if entry.generation == self.generation {
            self.busy = false;
            self.installing = false;
            self.native_terminal_received = true;
        }
        Reply::Retained {
            generation: entry.generation,
            outcome: entry.outcome,
        }
    }

    fn worker_stopped(&mut self) -> Option<Reply> {
        if self.stopped {
            return None;
        }
        self.stopped = true;
        self.commands.take();
        let installation_uncertain = self.installing && !self.native_terminal_received;
        let report = self.busy || !self.native_terminal_received;
        self.busy = false;
        self.installing = false;
        report.then_some(Reply::Stopped {
            generation: self.generation,
            installation_uncertain,
        })
    }

    fn spawn(&mut self) -> bool {
        let Some(events) = self.event_sender.take() else {
            return false;
        };
        let Some(repaint) = self.repaint.take() else {
            return false;
        };
        let (commands, receiver) = mpsc::channel();
        // NO_CLONE: the UI owner and independent update actor share cancellation
        // state; Arc copies only the handle and never database or UI payloads.
        let cancelled = Arc::clone(&self.cancelled);
        // NO_CLONE: native terminals must remain reachable while the actor owns
        // them, even if the replaceable progress receiver disconnects.
        let retained = Arc::clone(&self.retained);
        let result = thread::Builder::new()
            .name("tablecraft-update".into())
            .spawn(move || run(receiver, events, retained, cancelled, repaint));
        if result.is_err() {
            return false;
        }
        self.commands = Some(commands);
        true
    }
}

impl Drop for UpdateWorker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.commands.take();
        // Never join a potentially blocked HTTPS read on the UI thread.
        // Irreversible installation is separately guarded against window close.
    }
}

fn run(
    commands: Receiver<Task>,
    events: SyncSender<Reply>,
    retained: Arc<Mailbox<RetainedOutcome>>,
    cancelled: Arc<AtomicBool>,
    repaint: Box<dyn Fn() + Send>,
) {
    let mut session = None;
    for task in commands {
        if session.is_none() {
            match initialize() {
                Ok(initialized) => session = Some(initialized),
                Err(error) => {
                    if !post(
                        Reply::Failed {
                            generation: task.generation,
                            error,
                        },
                        &events,
                        &retained,
                        &*repaint,
                    ) {
                        break;
                    }
                    continue;
                }
            }
        }
        let Some(session) = session.as_mut() else {
            break;
        };
        let result = session.execute(
            task.command,
            &|| cancelled.load(Ordering::Acquire),
            &mut |event| {
                post(
                    Reply::Event {
                        generation: task.generation,
                        event,
                    },
                    &events,
                    &retained,
                    &*repaint,
                )
            },
        );
        if let Err(error) = result {
            if !post(
                Reply::Failed {
                    generation: task.generation,
                    error,
                },
                &events,
                &retained,
                &*repaint,
            ) {
                break;
            }
        }
    }
}

fn split_retained(reply: Reply) -> Result<RetainedEntry<RetainedOutcome>, Reply> {
    match reply {
        Reply::Event {
            generation,
            event: UpdateEvent::RestartScheduled(applied),
        } => Ok(RetainedEntry {
            generation,
            outcome: Arc::new(RetainedOutcome::RestartScheduled(applied)),
        }),
        Reply::Failed {
            generation,
            error: UpdateTaskError::Apply(error),
        } => Ok(RetainedEntry {
            generation,
            outcome: Arc::new(RetainedOutcome::ApplyFailed(error)),
        }),
        Reply::Retained {
            generation,
            outcome,
        } => Ok(RetainedEntry {
            generation,
            outcome,
        }),
        other => Err(other),
    }
}

fn post(
    reply: Reply,
    events: &SyncSender<Reply>,
    retained: &Mailbox<RetainedOutcome>,
    repaint: &dyn Fn(),
) -> bool {
    let reply = match split_retained(reply) {
        Ok(entry) => {
            retained.push(entry);
            repaint();
            return true;
        }
        Err(reply) => reply,
    };
    let progress = matches!(
        &reply,
        Reply::Event {
            event: UpdateEvent::Downloading(_),
            ..
        }
    );
    if progress {
        match events.try_send(reply) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(unsent)) => {
                return disconnected(unsent, retained, repaint);
            }
        }
    } else {
        match events.send(reply) {
            Ok(()) => {}
            Err(mpsc::SendError(unsent)) => return disconnected(unsent, retained, repaint),
        }
    }
    repaint();
    true
}

fn disconnected(unsent: Reply, retained: &Mailbox<RetainedOutcome>, repaint: &dyn Fn()) -> bool {
    match split_retained(unsent) {
        Ok(entry) => {
            retained.push(entry);
            repaint();
            true
        }
        // Only replaceable status/progress can be abandoned on a closed channel.
        Err(_) => false,
    }
}

fn initialize() -> Result<Session, UpdateTaskError> {
    let platform =
        Platform::detect().ok_or(UpdateTaskError::Operation(UpdateError::ReleaseMetadata))?;
    let version = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|_| UpdateTaskError::Operation(UpdateError::ReleaseMetadata))?;
    Session::new(version, platform).map_err(UpdateTaskError::Operation)
}

#[cfg(test)]
mod tests;
