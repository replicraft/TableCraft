//! Borrowed expected-root lookup selected before reading index membership.
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Lookup<'expected> {
    expected: &'expected [(u32, Option<i32>)],
    ordered: bool,
}

impl<'expected> Lookup<'expected> {
    pub(super) fn capture(
        expected: &'expected [(u32, Option<i32>)],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        check(budget, cancelled)?;
        let mut ordered = true;
        for pair in expected.windows(2) {
            check(budget, cancelled)?;
            if pair[0].0 > pair[1].0 {
                ordered = false;
                break;
            }
        }
        Ok(Self { expected, ordered })
    }

    pub(super) fn position(
        &self,
        reference: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<usize>, InspectError> {
        check(budget, cancelled)?;
        if !self.ordered {
            // Preserve the original full linear-search allowance, with a
            // cancellation check during the already paid comparisons.
            budget.charge(Resource::Steps, self.expected.len() as u64)?;
            for (position, (candidate, _)) in self.expected.iter().enumerate() {
                if cancelled() {
                    return Err(InspectError::Cancelled);
                }
                if *candidate == reference {
                    return Ok(Some(position));
                }
            }
            return Ok(None);
        }

        // Lower bound, rather than an arbitrary equal hit, preserves the original
        // first-match behavior for duplicate references and the caller's seen test.
        let mut start = 0;
        let mut end = self.expected.len();
        while start < end {
            check(budget, cancelled)?;
            let middle = start + (end - start) / 2;
            if self.expected[middle].0 < reference {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        Ok(self
            .expected
            .get(start)
            .filter(|(candidate, _)| *candidate == reference)
            .map(|_| start))
    }
}

#[cfg(test)]
mod tests;
