use semver::Version;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender, TrySendError},
    },
    thread,
};
use tablecraft_update::{
    UpdateError,
    platform::Platform,
    session::{Session, UpdateCommand, UpdateEvent, UpdateTaskError},
};

const EVENT_CAPACITY: usize = 8;

struct Task {
    generation: u64,
    command: UpdateCommand,
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
}

pub(crate) struct UpdateWorker {
    commands: Option<Sender<Task>>,
    events: Receiver<Reply>,
    event_sender: Option<SyncSender<Reply>>,
    cancelled: Arc<AtomicBool>,
    repaint: Option<Box<dyn Fn() + Send>>,
    generation: u64,
    busy: bool,
    installing: bool,
}

impl UpdateWorker {
    pub(crate) fn new(repaint: impl Fn() + Send + 'static) -> Self {
        let (event_sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        Self {
            commands: None,
            events,
            event_sender: Some(event_sender),
            cancelled: Arc::new(AtomicBool::new(false)),
            repaint: Some(Box::new(repaint)),
            generation: 0,
            busy: false,
            installing: false,
        }
    }

    pub(crate) fn start(&mut self, command: UpdateCommand) -> bool {
        if self.busy {
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
            let reply = self.events.try_recv().ok()?;
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
                Reply::Failed { generation, .. } => (*generation, true),
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

    fn spawn(&mut self) -> bool {
        let Some(events) = self.event_sender.take() else {
            return false;
        };
        let Some(repaint) = self.repaint.take() else {
            return false;
        };
        let (commands, receiver) = mpsc::channel();
        // NO_CLONE: the event thread and update worker share one cancellation
        // flag; Arc transfers only shared ownership, never database or UI data.
        let cancelled = Arc::clone(&self.cancelled);
        let result = thread::Builder::new()
            .name("tablecraft-update".into())
            .spawn(move || run(receiver, events, cancelled, repaint));
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
    cancelled: Arc<AtomicBool>,
    repaint: Box<dyn Fn() + Send>,
) {
    let mut session = None;
    for task in commands {
        if session.is_none() {
            match initialize() {
                Ok(initialized) => session = Some(initialized),
                Err(error) => {
                    if events
                        .send(Reply::Failed {
                            generation: task.generation,
                            error,
                        })
                        .is_err()
                    {
                        break;
                    }
                    repaint();
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
                let progress = matches!(&event, UpdateEvent::Downloading(_));
                let reply = Reply::Event {
                    generation: task.generation,
                    event,
                };
                let accepted = if progress {
                    match events.try_send(reply) {
                        Ok(()) | Err(TrySendError::Full(_)) => true,
                        Err(TrySendError::Disconnected(_)) => false,
                    }
                } else {
                    events.send(reply).is_ok()
                };
                if accepted {
                    repaint();
                }
                accepted
            },
        );
        if let Err(error) = result {
            if events
                .send(Reply::Failed {
                    generation: task.generation,
                    error,
                })
                .is_err()
            {
                break;
            }
            repaint();
        }
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
