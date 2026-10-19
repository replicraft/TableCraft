use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct CancelOnDrop {
    cancelled: Arc<AtomicBool>,
    armed: bool,
}

impl CancelOnDrop {
    pub fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled,
            armed: true,
        }
    }
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
}
