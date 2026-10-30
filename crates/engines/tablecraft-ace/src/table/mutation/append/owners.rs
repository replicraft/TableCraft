//! Immutable table identities for the shared INDEX append consumer.
pub(in crate::table) mod fresh_index;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck,
    format::{allocation, reader::Reader},
    header::check,
    table::mutation::map_authority::{self, Census, Owner},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) trait OwnerContext {
    fn single(&self) -> Result<u32, EditError>;
    fn index_definition(
        &self,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError>;
    fn census(&self, image: &NativeImage) -> Result<Option<&Census>, EditError>;
}

impl OwnerContext for u32 {
    fn single(&self) -> Result<u32, EditError> {
        Ok(*self)
    }

    fn index_definition(
        &self,
        _: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError> {
        check(budget, cancelled)?;
        Ok(*self)
    }

    fn census(&self, _: &NativeImage) -> Result<Option<&Census>, EditError> {
        Ok(None)
    }
}

struct Identity {
    definition: u32,
    physical: u32,
    root: u32,
    usage: u32,
}

pub(in crate::table) struct MultipleOwners {
    address: usize,
    length: usize,
    indexes: Vec<Identity>,
    census: Census,
}

impl MultipleOwners {
    pub(in crate::table) fn capture(
        image: &NativeImage,
        tables: &[&TableCheck],
        index_pages: &[u32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        // Map-record exclusivity alone does not exclude shared payload pages.
        // Check every authoritative foreign body claim before granting mutable
        // INDEX ownership. Candidate maps are hints, not body ownership.
        let census = map_authority::capture_catalog(
            image,
            budget,
            cancelled,
            |schema, indexes, columns, reader| {
                require_disjoint(schema.layout.usage, index_pages, reader)?;
                for column in &columns.entries {
                    reader.tick()?;
                    require_disjoint(column.references.owned(), index_pages, reader)?;
                }

                reader.work(tables.len())?;
                let selected = tables
                    .iter()
                    .any(|table| table.schema().definition_page() == schema.definition_page());
                if !selected {
                    for index in indexes {
                        reader.tick()?;
                        require_disjoint(index.usage_map(), index_pages, reader)?;
                    }
                }
                Ok(())
            },
        )?;
        let mut indexes = Vec::new();
        let mut roles = Vec::new();
        push(&mut roles, Owner::GlobalFree, budget)?;
        for table in tables {
            check(budget, cancelled)?;
            let definition = table.schema().definition_page();
            push(&mut roles, Owner::OrdinaryOwned { definition }, budget)?;
            push(&mut roles, Owner::OrdinaryCandidate { definition }, budget)?;
            for checked in table.indexes() {
                check(budget, cancelled)?;
                let index = checked.index();
                budget
                    .charge(Resource::Steps, indexes.len() as u64)
                    .map_err(InspectError::from)?;
                if indexes.iter().any(|prior: &Identity| {
                    prior.definition == definition && prior.physical == index.data_number()
                }) {
                    continue;
                }
                push(
                    &mut indexes,
                    Identity {
                        definition,
                        physical: index.data_number(),
                        root: index.root_page(),
                        usage: index.usage_map(),
                    },
                    budget,
                )?;
                push(
                    &mut roles,
                    Owner::IndexOwned {
                        definition,
                        physical: index.data_number(),
                    },
                    budget,
                )?;
            }
        }
        census.require_exclusive(&roles, budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            indexes,
            census,
        })
    }
}

impl OwnerContext for MultipleOwners {
    fn single(&self) -> Result<u32, EditError> {
        Err(EditError::Preservation)
    }

    fn index_definition(
        &self,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u32, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.indexes.len() as u64)
            .map_err(InspectError::from)?;
        let mut found = None;
        for identity in &self.indexes {
            if identity.physical == index.data_number()
                && identity.root == index.root_page()
                && identity.usage == index.usage_map()
            {
                if found.is_some() {
                    return Err(EditError::Preservation);
                }
                found = Some(identity.definition);
            }
        }
        found.ok_or(EditError::Preservation)
    }

    fn census(&self, image: &NativeImage) -> Result<Option<&Census>, EditError> {
        // This binding is operation-local; no saved address is dereferenced.
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
        {
            return Err(EditError::Preservation);
        }
        Ok(Some(&self.census))
    }
}

fn require_disjoint(
    reference: u32,
    selected: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    for page in allocation::pages(reader, reference)? {
        reader.tick()?;
        reader.work(selected.len())?;
        if selected.contains(&page) {
            return Err(EditError::Unsupported(crate::WriteBlocker::IndexedColumn));
        }
    }
    Ok(())
}

fn push<T>(items: &mut Vec<T>, item: T, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    items.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    items.push(item);
    Ok(())
}
