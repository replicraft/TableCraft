//! Process-cached system names discovered once away from the event thread.
#[cfg(test)]
mod tests;

use egui::Context;
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tablecraft_ui::FontFamilies;

static CATALOG: Catalog = Catalog::new();
const CHECK_INTERVAL: Duration = Duration::from_millis(100);
type Task = Box<dyn FnOnce() + Send + 'static>;

struct Catalog {
    started: AtomicBool,
    failed: AtomicBool,
    families: OnceLock<FontFamilies>,
}

impl Catalog {
    const fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            families: OnceLock::new(),
        }
    }

    fn start_with(
        &'static self,
        context: &Context,
        discover: impl FnOnce() -> Option<FontFamilies> + Send + 'static,
        spawn: impl FnOnce(Task) -> io::Result<()>,
    ) {
        if self
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        // Background font discovery needs one independently owned Context for completion
        // repaint; the borrowed caller Context remains available for immediate spawn failure.
        // Context shares egui state rather than copying the UI.
        let repaint = context.clone();
        let task = Box::new(move || {
            match catch_unwind(AssertUnwindSafe(discover)) {
                Ok(Some(families)) => {
                    let _ = self.families.set(families);
                }
                Ok(None) | Err(_) => {
                    self.failed.store(true, Ordering::Release);
                    tracing::warn!(
                        "system font family discovery failed; bundled fonts remain available"
                    );
                }
            }
            // A closed event loop must not turn inert discovery into a panic.
            let _ = catch_unwind(AssertUnwindSafe(|| repaint.request_repaint()));
        });
        if spawn(task).is_err() {
            self.failed.store(true, Ordering::Release);
            tracing::warn!(
                "system font discovery worker could not start; bundled fonts remain available"
            );
            context.request_repaint();
        }
    }
}

/// Per-host observation only; the process owns one bounded discovery task/cache.
#[derive(Default)]
pub(super) struct FontInventory {
    finished: bool,
}

impl FontInventory {
    pub(super) fn poll(&mut self, context: &Context) -> Option<FontFamilies> {
        if self.finished {
            return None;
        }
        CATALOG.start_with(context, discover, spawn);
        self.poll_catalog(&CATALOG, context)
    }

    fn poll_catalog(&mut self, catalog: &Catalog, context: &Context) -> Option<FontFamilies> {
        if self.finished {
            return None;
        }
        // get() never waits for OnceLock initialization. Only the background
        // owner calls set(); no UI constructor runs discovery or get_or_init.
        if let Some(families) = catalog.families.get() {
            self.finished = true;
            context.request_repaint();
            // The process OnceLock retains the canonical Arc<[String]> catalog; a host obtains
            // a cheap shared owned FontFamilies without copying all family strings.
            return Some(families.clone());
        }
        if catalog.failed.load(Ordering::Acquire) {
            self.finished = true;
        } else {
            // A second live host does not need another retained event-loop
            // callback or unbounded waiter list to observe the process cache.
            context.request_repaint_after(CHECK_INTERVAL);
        }
        None
    }
}

fn spawn(task: Task) -> io::Result<()> {
    std::thread::Builder::new()
        .name("tablecraft-font-inventory".into())
        .spawn(task)
        .map(drop)
}

fn discover() -> Option<FontFamilies> {
    let _span = tracing::debug_span!("font_families_discover").entered();
    let mut collection = fontique::Collection::new(fontique::CollectionOptions::default());
    FontFamilies::from_system(collection.family_names()).ok()
}
