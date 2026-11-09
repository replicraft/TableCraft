//! Fallible OS thread startup shared by resource-owning workers.
use std::io;

pub(crate) trait Spawner {
    fn spawn(self, task: impl FnOnce() + Send + 'static) -> io::Result<()>;
}

pub(crate) struct ThreadSpawner;

impl Spawner for ThreadSpawner {
    fn spawn(self, task: impl FnOnce() + Send + 'static) -> io::Result<()> {
        std::thread::Builder::new().spawn(task).map(drop)
    }
}

#[cfg(test)]
pub(crate) struct FailingSpawner;

#[cfg(test)]
impl Spawner for FailingSpawner {
    fn spawn(self, _task: impl FnOnce() + Send + 'static) -> io::Result<()> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

/// Holds an actor before its first mailbox read, exposing pre-wait shutdown.
#[cfg(test)]
pub(crate) struct PausedSpawner {
    gate: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
impl PausedSpawner {
    pub(crate) fn new() -> (Self, std::sync::mpsc::SyncSender<()>) {
        let (release, gate) = std::sync::mpsc::sync_channel(1);
        (Self { gate }, release)
    }
}

#[cfg(test)]
impl Spawner for PausedSpawner {
    fn spawn(self, task: impl FnOnce() + Send + 'static) -> io::Result<()> {
        ThreadSpawner.spawn(move || {
            self.gate.recv().expect("test releases the paused actor");
            task();
        })
    }
}
