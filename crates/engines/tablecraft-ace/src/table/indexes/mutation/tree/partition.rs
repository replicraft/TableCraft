//! Assign final sorted survivors by original leaf survivor capacities.
use super::Tree;
use crate::table::indexes::page::KeyRef;
use crate::{EditError, InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl Tree {
    pub(in crate::table::indexes::mutation) fn partition_survivors<K: Copy>(
        &self,
        original: &mut &[KeyRef<K>],
        surviving: &mut &[KeyRef<K>],
        removed: &[u32],
        output: &mut Vec<KeyRef<K>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if !self.children.is_empty() {
            for child in &self.children {
                child
                    .partition_survivors(original, surviving, removed, output, budget, cancelled)?;
            }
            return Ok(());
        }
        let (source, rest) = original
            .split_at_checked(self.count)
            .ok_or(EditError::Preservation)?;
        *original = rest;
        let mut count = 0;
        for key in source {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, removed.len() as u64)
                .map_err(InspectError::from)?;
            count += usize::from(!removed.contains(&key.row));
        }
        let (assigned, rest) = surviving
            .split_at_checked(count)
            .ok_or(EditError::Preservation)?;
        *surviving = rest;
        output.extend_from_slice(assigned);
        // Removed original references are positional certificates consumed by
        // contraction, never serialized. Every leaf still consumes old count.
        for key in source {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, removed.len() as u64)
                .map_err(InspectError::from)?;
            if removed.contains(&key.row) {
                output.push(*key);
            }
        }
        Ok(())
    }
}
