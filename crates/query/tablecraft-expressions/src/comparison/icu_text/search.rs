use super::IcuTextCollation;
use crate::{EvalError, comparison::StringSearch};
use std::{cmp::Ordering, ops::Range};
use tablecraft_core::limits::ResourceBudget;

impl IcuTextCollation<'_> {
    fn boundaries(
        &self,
        input: &str,
        budget: &mut ResourceBudget,
    ) -> Result<Vec<(usize, usize)>, EvalError> {
        if input.len() > self.limits.maximum_input_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut units = 0usize;
        let mut count = 1usize;
        for character in input.chars() {
            self.check(budget, 1)?;
            units = units
                .checked_add(character.len_utf16())
                .ok_or(EvalError::Overflow)?;
            if units > self.limits.maximum_search_units {
                return Err(EvalError::TextLimit);
            }
            count = count.checked_add(1).ok_or(EvalError::Allocation)?;
        }
        let charged_count = u64::try_from(count).map_err(|_| EvalError::Allocation)?;
        budget.charge_allocation(charged_count, size_of::<(usize, usize)>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| EvalError::Allocation)?;
        let mut units = 0;
        for (byte, character) in input.char_indices() {
            self.check(budget, 1)?;
            result.push((byte, units));
            units += character.len_utf16();
        }
        result.push((input.len(), units));
        Ok(result)
    }

    pub(super) fn search(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if (self.cancelled)() {
            return Err(EvalError::Cancelled);
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        self.check(&mut budget, 1)?;
        let boundaries = self.boundaries(haystack, &mut budget)?;
        // Count/validate needle separately; no temporary substring or key vector.
        if needle.len() > self.limits.maximum_input_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut needle_units = 0;
        for character in needle.chars() {
            self.check(&mut budget, 1)?;
            needle_units += character.len_utf16();
            if needle_units > self.limits.maximum_search_units {
                return Err(EvalError::TextLimit);
            }
        }
        if haystack.is_empty() || needle.is_empty() {
            return Ok(None);
        }
        let count = boundaries.len() - 1;
        for position in 0..count {
            self.check(&mut budget, 1)?;
            let start = match search {
                StringSearch::ForwardFrom(_) => position,
                StringSearch::ReverseThrough(_) => count - position - 1,
            };
            let (byte_start, unit_start) = boundaries[start];
            let admitted = match search {
                StringSearch::ForwardFrom(first) => unit_start >= first,
                StringSearch::ReverseThrough(end) => unit_start < end,
            };
            if !admitted {
                continue;
            }
            for &(byte_end, unit_end) in &boundaries[start + 1..] {
                self.check(&mut budget, 1)?;
                if matches!(search, StringSearch::ReverseThrough(end) if unit_end > end) {
                    break;
                }
                if self.compare_checked(&haystack[byte_start..byte_end], needle, &mut budget)?
                    == Ordering::Equal
                {
                    return Ok(Some(unit_start..unit_end));
                }
            }
        }
        Ok(None)
    }
}
