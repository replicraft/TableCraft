use super::{Command, Error, Response, read::Document};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

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
        let mut document = None;
        Self::start(repaint, move |command, cancelled| match command {
            Command::Open { path, password } => {
                // A failed open cannot leave the old file behind a new UI title.
                document = None;
                let (opened, response) = Document::open(path, password, cancelled)?;
                document = Some(opened);
                Ok(response)
            }
            Command::Page { table, offset } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .page(table, *offset, cancelled),
            Command::Query { sql } => document
                .as_mut()
                .ok_or(Error::NoDatabase)?
                .query(sql, cancelled),
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
        std::thread::spawn(move || {
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
                let result = execute(&mut request.command, &cancelled);
                if !cancelled() {
                    *thread_shared.response.lock().unwrap() = Some((request.id, result));
                    repaint();
                }
            }
        });
        Self { shared }
    }

    /// Supersede earlier work without waiting for that work to finish.
    pub fn request(&self, command: Command) -> u64 {
        let id = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *self.shared.pending.lock().unwrap() = Some(Pending { id, command });
        self.shared.response.lock().unwrap().take();
        self.shared.ready.notify_one();
        id
    }

    pub fn poll(&self) -> Option<Reply> {
        let response = self.shared.response.lock().unwrap().take()?;
        (response.0 == self.shared.generation.load(Ordering::Acquire)).then_some(response)
    }

    pub fn cancel(&self) {
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.pending.lock().unwrap().take();
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
