//! Physical EOF closure and every referenced inline-map record, not provenance.
use super::{records, refused};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::u32_at,
        page::PAGE_BYTES,
        row::{reference, reference_page, reference_slot},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_PAGES: usize = u64::BITS as usize;
const INLINE_TAG_OFFSET: usize = 0;
const INLINE_BASE_OFFSET: usize = size_of::<u8>();
const INLINE_BITMAP_OFFSET: usize = INLINE_BASE_OFFSET + size_of::<u32>();
const INLINE_BYTES: usize = INLINE_BITMAP_OFFSET + size_of::<u64>();
const INLINE_TAG: u8 = 0;
const GLOBAL_PAGE: u32 = 1;
const GLOBAL_SLOT: u8 = 0;
const GLOBAL_OWNER: u32 = 1;
const ORDINARY_MAP_OWNER: u32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Unclaimed,
    Header,
    Definition,
    Data,
    Maps,
}

pub(super) struct Ledger<'image> {
    image: &'image NativeImage,
    roles: Vec<Role>,
    references: Vec<u32>,
    free: u64,
}

impl<'image> Ledger<'image> {
    pub fn new(
        image: &'image NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let length = image.as_bytes().len();
        if !length.is_multiple_of(PAGE_BYTES) {
            return Err(refused());
        }
        let pages = length / PAGE_BYTES;
        if !(3..=MAX_PAGES).contains(&pages) {
            return Err(refused());
        }
        budget
            .charge_allocation(pages as u64, size_of::<Role>() as u64)
            .map_err(InspectError::from)?;
        let mut roles = Vec::new();
        roles
            .try_reserve_exact(pages)
            .map_err(|_| InspectError::Allocation)?;
        roles.resize(pages, Role::Unclaimed);
        let mut result = Self {
            image,
            roles,
            references: Vec::new(),
            free: 0,
        };
        result.claim(0, Role::Header, budget, cancelled)?;
        result.free = result.map(reference(GLOBAL_PAGE, GLOBAL_SLOT), budget, cancelled)?;
        if pages < MAX_PAGES && result.free >> pages != 0 {
            return Err(refused());
        }
        Ok(result)
    }

    pub fn page(&self, number: u32) -> Result<&'image [u8], EditError> {
        let start = usize::try_from(number)
            .ok()
            .and_then(|number| number.checked_mul(PAGE_BYTES))
            .ok_or_else(refused)?;
        let end = start.checked_add(PAGE_BYTES).ok_or_else(refused)?;
        self.image.as_bytes().get(start..end).ok_or_else(refused)
    }

    pub fn claim(
        &mut self,
        number: u32,
        role: Role,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        let slot = self.roles.get_mut(number as usize).ok_or_else(refused)?;
        if *slot != Role::Unclaimed && !(*slot == Role::Maps && role == Role::Maps) {
            return Err(refused());
        }
        *slot = role;
        Ok(())
    }

    pub fn map(
        &mut self,
        value: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.references.len() as u64)
            .map_err(InspectError::from)?;
        if value == 0 || self.references.contains(&value) {
            return Err(refused());
        }
        let number = reference_page(value);
        if number == GLOBAL_PAGE && value != reference(GLOBAL_PAGE, GLOBAL_SLOT) {
            return Err(refused());
        }
        self.claim(number, Role::Maps, budget, cancelled)?;
        let page = self.page(number)?;
        let owner = if number == GLOBAL_PAGE {
            GLOBAL_OWNER
        } else {
            ORDINARY_MAP_OWNER
        };
        let count = records::count(page, owner, budget, cancelled)?;
        let slot = usize::from(reference_slot(value));
        if slot >= count || (number == GLOBAL_PAGE && count != 1) {
            return Err(refused());
        }
        let record = records::row(page, slot)?;
        budget
            .charge(Resource::Steps, INLINE_BYTES as u64)
            .map_err(InspectError::from)?;
        if record.len() != INLINE_BYTES
            || record[INLINE_TAG_OFFSET] != INLINE_TAG
            || u32_at(record, INLINE_BASE_OFFSET)? != 0
        {
            return Err(refused());
        }
        let bitmap = u64::from_le_bytes(record[INLINE_BITMAP_OFFSET..].try_into().unwrap());
        budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        self.references
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        self.references.push(value);
        Ok(bitmap)
    }

    pub fn members(
        &self,
        bitmap: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, EditError> {
        budget
            .charge_allocation(u64::from(bitmap.count_ones()), size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(bitmap.count_ones() as usize)
            .map_err(|_| InspectError::Allocation)?;
        for number in 0..MAX_PAGES {
            check(budget, cancelled)?;
            if bitmap & (1_u64 << number) == 0 {
                continue;
            }
            if number >= self.roles.len() {
                return Err(refused());
            }
            result.push(number as u32);
        }
        Ok(result)
    }

    pub fn finish(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for (number, role) in self.roles.iter().enumerate() {
            check(budget, cancelled)?;
            let free = self.free & (1_u64 << number) != 0;
            if *role == Role::Unclaimed {
                if !free {
                    return Err(refused());
                }
                budget
                    .charge(Resource::Steps, PAGE_BYTES as u64)
                    .map_err(InspectError::from)?;
                if self.page(number as u32)?.iter().any(|byte| *byte != 0) {
                    return Err(refused());
                }
            } else if free {
                return Err(refused());
            }
            if *role != Role::Maps {
                continue;
            }
            let page = self.page(number as u32)?;
            let owner = if number as u32 == GLOBAL_PAGE {
                GLOBAL_OWNER
            } else {
                ORDINARY_MAP_OWNER
            };
            let count = records::count(page, owner, budget, cancelled)?;
            for slot in 0..count {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, self.references.len() as u64)
                    .map_err(InspectError::from)?;
                if !self
                    .references
                    .contains(&reference(number as u32, slot as u8))
                {
                    return Err(refused());
                }
            }
        }
        Ok(())
    }
}
