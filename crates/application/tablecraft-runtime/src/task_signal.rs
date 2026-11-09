//! Durable nonblocking wakeups for mailbox-owning application actors.
use std::sync::mpsc::{self, SyncSender, TrySendError};

pub(crate) struct Signal {
    sender: SyncSender<()>,
}

pub(crate) struct Receiver {
    receiver: mpsc::Receiver<()>,
}

pub(crate) fn channel() -> (Signal, Receiver) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (Signal { sender }, Receiver { receiver })
}

impl Signal {
    pub(crate) fn notify(&self) {
        match self.sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => {}
            Err(TrySendError::Disconnected(())) => {}
        }
    }
}

impl Receiver {
    /// Only an actor waits; application callers never block on this channel.
    pub(crate) fn wait(&self) -> bool {
        self.receiver.recv().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::channel;

    #[test]
    fn notification_before_wait_is_retained_and_coalesces() {
        let (signal, receiver) = channel();
        signal.notify();
        signal.notify();
        drop(signal);
        assert!(receiver.wait());
        assert!(!receiver.wait());
    }

    #[test]
    fn notification_after_receiver_teardown_does_not_block_or_panic() {
        let (signal, receiver) = channel();
        drop(receiver);
        signal.notify();
    }
}
