//! Complete original metadata footprints deny DATA writes, not map-cell edits.
use super::super::{EditError, WriteBlocker, map_authority};
use crate::{
    IndexCheck, InspectError, NativeImage, NativeIndex, NativeReadSource, TableSchema,
    format::{definition, reader::Reader},
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

#[cfg(test)]
mod tests;

pub(in crate::table::mutation) struct Metadata {
    protected_pages: Vec<u32>,
}

impl Metadata {
    pub(in crate::table::mutation) fn capture(
        image: &NativeImage,
        schema: &TableSchema,
        indexes: &[IndexCheck],
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let number = schema
            .columns()
            .get(column)
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?
            .number();
        let definition = schema.definition_page();
        let shared = [
            map_authority::Owner::GlobalFree,
            map_authority::Owner::OrdinaryOwned { definition },
            map_authority::Owner::OrdinaryCandidate { definition },
        ];
        let capacity = indexes
            .len()
            .checked_add(shared.len())
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge_allocation(capacity as u64, size_of::<map_authority::Owner>() as u64)
            .map_err(InspectError::from)?;
        let mut mutable = Vec::new();
        mutable
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
        mutable.extend_from_slice(&shared);
        let mut protected_pages = Vec::new();
        let mut selected_found = false;
        let census = map_authority::capture_catalog(
            image,
            budget,
            cancelled,
            |local, native_indexes, _, reader| {
                definition::read_with_pages(reader, local.definition_page(), |page, reader| {
                    protect(&mut protected_pages, page, reader)
                })?;
                if local.definition_page() != definition {
                    return Ok(());
                }
                selected_found = true;
                reader.work(local.columns().len())?;
                if !local.columns().iter().any(|field| field.number() == number)
                    || native_indexes.len() != indexes.len()
                {
                    return Err(EditError::Preservation);
                }
                for native in native_indexes {
                    reader.tick()?;
                    reader.work(indexes.len() + native.columns().len())?;
                    let checked = indexes
                        .iter()
                        .find(|checked| checked.index().number() == native.number())
                        .ok_or(EditError::Preservation)?;
                    if !same_index(native, checked.index()) {
                        return Err(EditError::Preservation);
                    }
                    if !native
                        .columns()
                        .iter()
                        .any(|field| field.number() == number)
                    {
                        continue;
                    }
                    let owner = map_authority::Owner::IndexOwned {
                        definition,
                        physical: native.data_number(),
                    };
                    reader.work(mutable.len())?;
                    if !mutable.contains(&owner) {
                        mutable.push(owner);
                    }
                }
                Ok(())
            },
        )?;
        if !selected_found {
            return Err(EditError::Preservation);
        }
        census.require_exclusive(&mutable, budget, cancelled)?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        census.protect_storage(&mut protected_pages, &mut reader)?;
        Ok(Self { protected_pages })
    }

    /// Deny only original definition/map storage. This grants no DATA owner,
    /// current hint, allocation or geometry authority for old or staged pages.
    pub(in crate::table::mutation) fn require_data_page(
        &self,
        number: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        for &page in &self.protected_pages {
            check(budget, cancelled)?;
            if number == page {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

fn same_index(left: &NativeIndex, right: &NativeIndex) -> bool {
    left.number() == right.number()
        && left.data_number() == right.data_number()
        && left.root_page() == right.root_page()
        && left.usage_map() == right.usage_map()
        && left.native_type() == right.native_type()
        && left.flags() == right.flags()
        && left.columns() == right.columns()
}

fn protect<R: NativeReadSource>(
    pages: &mut Vec<u32>,
    number: u32,
    reader: &mut Reader<'_, R>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.work(pages.len())?;
    if pages.contains(&number) {
        return Ok(());
    }
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)?;
    pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    pages.push(number);
    Ok(())
}
