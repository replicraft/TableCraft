//! Selected retained INDEX members must have one actual physical storage role.
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{allocation, definition, reader::Reader},
    header::check,
    table::mutation::{
        lval_ownership::Inventory,
        map_authority::{self, Owner},
    },
};
use std::{io::Cursor, slice};
use tablecraft_core::limits::ResourceBudget;

#[cfg(test)]
mod tests;

struct Member {
    owner: Owner,
    number: u32,
}

struct Roles {
    selected: u32,
    members: Vec<Member>,
    metadata: Vec<u32>,
    selected_owners: Vec<Owner>,
}

fn reserve<T>(
    values: &mut Vec<T>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    reader.tick()?;
    if values.len() == values.capacity() {
        let relocation_bytes = values
            .len()
            .checked_mul(size_of::<T>())
            .ok_or(InspectError::StructureLimit)?;
        reader.work(relocation_bytes)?;
    }
    reader.budget.charge_allocation(1, size_of::<T>() as u64)?;
    values
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)
}

impl Roles {
    fn selected_index(&self, owner: Owner) -> bool {
        matches!(owner, Owner::IndexOwned { definition, .. } if definition == self.selected)
    }

    fn member(
        &mut self,
        owner: Owner,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        reader.work(self.members.len())?;
        for prior in &self.members {
            if prior.number != number {
                continue;
            }
            if prior.owner == owner {
                // Logical aliases of one physical data number are one owner.
                return Ok(());
            }
            if self.selected_index(owner) || self.selected_index(prior.owner) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        reserve(&mut self.members, reader)?;
        self.members.push(Member { owner, number });
        Ok(())
    }

    fn map(
        &mut self,
        owner: Owner,
        reference: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.selected_index(owner) {
            reader.tick()?;
            reader.work(self.selected_owners.len())?;
            if !self.selected_owners.contains(&owner) {
                reserve(&mut self.selected_owners, reader)?;
                self.selected_owners.push(owner);
            }
        }
        for number in allocation::pages(reader, reference)? {
            self.member(owner, number, reader)?;
        }
        Ok(())
    }

    fn protect(
        &mut self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), InspectError> {
        reader.tick()?;
        reader.work(self.metadata.len())?;
        if !self.metadata.contains(&number) {
            reserve(&mut self.metadata, reader)?;
            self.metadata.push(number);
        }
        Ok(())
    }
}

/// Inventory actual local ownership, not caller-provided role declarations.
/// Candidate map members are hints and may legitimately overlap owned members;
/// their storage pages are still excluded by Census::protect_storage below.
pub(super) fn require(
    image: &NativeImage,
    selected: u32,
    long_values: &Inventory,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.retained_index.member_roles").entered();
    check(budget, cancelled)?;
    let mut roles = Roles {
        selected,
        members: Vec::new(),
        metadata: Vec::new(),
        selected_owners: Vec::new(),
    };
    let census = map_authority::capture_catalog(
        image,
        budget,
        cancelled,
        |schema, indexes, declarations, reader| {
            let definition = schema.definition_page();
            roles.protect(0, reader)?;
            definition::read_with_pages(reader, definition, |number, reader| {
                roles.protect(number, reader)
            })?;
            roles.map(
                Owner::OrdinaryOwned { definition },
                schema.layout.usage,
                reader,
            )?;
            for index in indexes {
                reader.tick()?;
                roles.map(
                    Owner::IndexOwned {
                        definition,
                        physical: index.data_number(),
                    },
                    index.usage_map(),
                    reader,
                )?;
            }
            for declaration in &declarations.entries {
                reader.tick()?;
                roles.map(
                    Owner::ColumnOwned {
                        definition,
                        column: declaration.number,
                    },
                    declaration.references.owned(),
                    reader,
                )?;
            }
            Ok(())
        },
    )?;
    census.require_exclusive(&roles.selected_owners, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    census.protect_storage(&mut roles.metadata, &mut reader)?;
    for member in &roles.members {
        reader.tick()?;
        if !roles.selected_index(member.owner) {
            continue;
        }
        reader.work(roles.metadata.len())?;
        if roles.metadata.contains(&member.number) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        // Reuse the original Source's actual declaration + live-reference
        // inventory, including references not described by an owned bitmap.
        long_values.require_no_long_value_data_overlap(
            slice::from_ref(&member.number),
            reader.budget,
            cancelled,
        )?;
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
