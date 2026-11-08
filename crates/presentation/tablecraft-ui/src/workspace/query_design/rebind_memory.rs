//! Only source-free design choices are copied during a fresh-source transfer.
use super::{QueryDesign, retention::count_draft};
use crate::workspace::objects::memory::Counter;

impl QueryDesign {
    pub(in crate::workspace) fn rebind_preference_bytes(&self) -> Option<usize> {
        let Some(session) = &self.session else {
            return Some(0);
        };
        let mut count = Counter::new(size_of_val(session.as_ref()));
        count_draft(&mut count, &session.draft);
        count.finish()
    }
}
