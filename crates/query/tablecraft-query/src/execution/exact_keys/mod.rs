//! Optional exact-key acceleration; retained rows and SQL comparisons stay authoritative.
mod directory;
mod profile;

use crate::{
    QueryError,
    execution::{Runtime, cell::CellValue},
    source::TextPolicy,
};
use directory::Directory;
use profile::Kind;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct ExactKeys {
    profile: Vec<Kind>,
    directory: Directory,
    enabled: bool,
}

impl ExactKeys {
    pub(super) fn find(
        &self,
        fingerprint: u64,
        visit: impl FnMut(usize, bool) -> Result<bool, QueryError>,
    ) -> Result<Option<usize>, QueryError> {
        self.directory.find(fingerprint, visit)
    }

    pub(super) fn insert(
        &mut self,
        fingerprint: u64,
        representative: usize,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        self.directory
            .insert(fingerprint, representative, budget, cancelled)
    }

    pub(super) fn fingerprint(
        &mut self,
        row: &[CellValue],
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Option<u64>, QueryError> {
        if !self.enabled {
            return Ok(None);
        }
        let fingerprint = profile::fingerprint(&self.profile, row, budget, cancelled)?;
        if fingerprint.is_none() {
            // A descriptor is a successful-result guarantee, not permission to
            // reinterpret an unexpected cell. Keep every retained representative
            // visible to the original comparator for this and all later rows.
            self.enabled = false;
            self.directory = Directory::default();
        }
        Ok(fingerprint)
    }
}

impl Runtime<'_, '_> {
    pub(super) fn exact_key_match<T>(
        &mut self,
        lookup: &mut Option<ExactKeys>,
        key: &[CellValue],
        retained: &[T],
        cells: impl for<'r> Fn(&'r T) -> &'r [CellValue],
        policies: &[TextPolicy],
    ) -> Result<(Option<usize>, Option<u64>), QueryError> {
        let fingerprint = match lookup {
            Some(lookup) => lookup.fingerprint(key, self.budget, self.options.cancelled)?,
            None => None,
        };
        let representative =
            if let (Some(lookup), Some(fingerprint)) = (lookup.as_ref(), fingerprint) {
                lookup.find(fingerprint, |position, same_fingerprint| {
                    self.tick()?;
                    Ok(same_fingerprint
                        && self.same_expression_row(key, cells(&retained[position]), policies)?)
                })?
            } else {
                let mut representative = None;
                for (position, row) in retained.iter().enumerate() {
                    if self.same_expression_row(key, cells(row), policies)? {
                        representative = Some(position);
                        break;
                    }
                }
                representative
            };
        Ok((representative, fingerprint))
    }
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
