//! Physical EOF closure and every referenced inline-map record, not provenance.
use super::{inline::InlineBitmap, records, refused};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        allocation::GLOBAL_FREE_REFERENCE,
        page::PAGE_BYTES,
        row::{reference, reference_page, reference_slot},
    },
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MIN_PAGES: usize = 3;
const GLOBAL_PAGE: u32 = reference_page(GLOBAL_FREE_REFERENCE);
const GLOBAL_OWNER: u32 = GLOBAL_PAGE;
const ORDINARY_MAP_OWNER: u32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Unclaimed,
    Header,
    Definition,
    Data,
    Index,
    Maps,
}

pub(super) struct Ledger<'image> {
    image: &'image NativeImage,
    roles: Vec<Role>,
    references: Vec<u32>,
    free: InlineBitmap<'image>,
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
        if pages < MIN_PAGES || u32::try_from(pages).is_err() {
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
            free: InlineBitmap::empty(),
        };
        result.claim(0, Role::Header, budget, cancelled)?;
        result.free = result.map(GLOBAL_FREE_REFERENCE, budget, cancelled)?;
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
    ) -> Result<InlineBitmap<'image>, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.references.len() as u64)
            .map_err(InspectError::from)?;
        if value == 0 || self.references.contains(&value) {
            return Err(refused());
        }
        let number = reference_page(value);
        if number == GLOBAL_PAGE && value != GLOBAL_FREE_REFERENCE {
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
        let bitmap = InlineBitmap::capture(record, budget, cancelled)?;
        if value != GLOBAL_FREE_REFERENCE {
            bitmap.require_existing(self.roles.len(), budget, cancelled)?;
        }
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
        bitmap: InlineBitmap<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u32>, EditError> {
        bitmap.require_existing(self.roles.len(), budget, cancelled)?;
        bitmap.members(budget, cancelled)
    }

    pub fn finish(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for (number, role) in self.roles.iter().enumerate() {
            check(budget, cancelled)?;
            // Future-free capacity outside physical EOF never supplies a role.
            // Every actual page still has the ordinary polarity checks below.
            let free = self.free.bit(number as u32).ok_or_else(refused)?;
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
