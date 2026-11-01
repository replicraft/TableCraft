//! Borrowed native inline-map windows; map bytes never become page authority.
use super::refused;
use crate::{
    EditError, InspectError,
    format::{
        allocation::{INLINE_BITMAP_OFFSET, INLINE_MAP},
        bytes::u32_at,
        row::MASK_BITS,
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const TAG_OFFSET: usize = 0;
const BASE_OFFSET: usize = size_of::<u8>();

#[derive(Clone, Copy)]
pub(super) struct InlineBitmap<'image> {
    base: u32,
    bytes: &'image [u8],
}

impl<'image> InlineBitmap<'image> {
    /// Initialization value only. The GLOBAL cell must replace this before a
    /// ledger can be returned; an empty bitmap does not establish any role.
    pub const fn empty() -> Self {
        Self {
            base: 0,
            bytes: &[],
        }
    }

    pub fn capture(
        record: &'image [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        if record.len() < INLINE_BITMAP_OFFSET || record[TAG_OFFSET] != INLINE_MAP {
            return Err(refused());
        }
        budget
            .charge(Resource::Steps, INLINE_BITMAP_OFFSET as u64)
            .map_err(InspectError::from)?;
        let bitmap = Self {
            base: u32_at(record, BASE_OFFSET)?,
            bytes: &record[INLINE_BITMAP_OFFSET..],
        };
        bitmap.walk(budget, cancelled, |_| Ok(()))?;
        Ok(bitmap)
    }

    /// Owned/candidate members name actual input pages. GLOBAL FREE capacity
    /// uses a different polarity and deliberately does not invoke this rule.
    pub fn require_existing(
        self,
        page_count: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.walk(budget, cancelled, |page| {
            if u64::from(page) >= page_count as u64 {
                return Err(refused());
            }
            Ok(())
        })
    }

    /// A missing cell is distinct from an explicit zero bit. In particular,
    /// GLOBAL allocation cannot infer an allocated page outside its window.
    pub fn bit(self, page: u32) -> Option<bool> {
        let relative = usize::try_from(page.checked_sub(self.base)?).ok()?;
        self.bytes
            .get(relative / MASK_BITS)
            .map(|byte| byte & (1 << (relative % MASK_BITS)) != 0)
    }

    pub fn contains(self, page: u32) -> bool {
        self.bit(page) == Some(true)
    }

    pub fn is_empty(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        for byte in self.bytes {
            check(budget, cancelled)?;
            if *byte != 0 {
                return Ok(false);
            }
        }
        check(budget, cancelled)?;
        Ok(true)
    }

    /// Windows may have different bases and lengths. Only actual set-page
    /// membership determines the candidate-subset law.
    pub fn is_subset_of(
        self,
        owned: Self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        let mut subset = true;
        self.walk(budget, cancelled, |page| {
            subset &= owned.contains(page);
            Ok(())
        })?;
        Ok(subset)
    }

    pub fn members(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, EditError> {
        let mut count = 0_usize;
        self.walk(budget, cancelled, |_| {
            count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
            Ok(())
        })?;
        budget
            .charge_allocation(count as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        self.walk(budget, cancelled, |page| {
            members.push(page);
            Ok(())
        })?;
        Ok(members)
    }

    fn walk(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
        mut visit: impl FnMut(u32) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        for (offset, byte) in self.bytes.iter().copied().enumerate() {
            check(budget, cancelled)?;
            if byte == 0 {
                continue;
            }
            for bit in 0..MASK_BITS {
                check(budget, cancelled)?;
                if byte & (1 << bit) == 0 {
                    continue;
                }
                let relative = offset
                    .checked_mul(MASK_BITS)
                    .and_then(|relative| relative.checked_add(bit))
                    .and_then(|relative| u32::try_from(relative).ok())
                    .ok_or(InspectError::StructureLimit)?;
                let page = self.base.checked_add(relative).ok_or_else(refused)?;
                visit(page)?;
            }
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
