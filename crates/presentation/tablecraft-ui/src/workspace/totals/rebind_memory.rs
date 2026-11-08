//! Totals choices transfer without cached totals, descriptions or pending work.
use super::Totals;
use crate::workspace::objects::memory::Counter;

impl Totals {
    pub(in crate::workspace) fn rebind_preference_bytes(&self) -> Option<usize> {
        let Some(session) = &self.session else {
            return Some(0);
        };
        let mut count = Counter::new(size_of_val(session.as_ref()));
        count.vector(&session.specs);
        for spec in &session.specs {
            count.string(&spec.column);
        }
        count.finish()
    }
}
