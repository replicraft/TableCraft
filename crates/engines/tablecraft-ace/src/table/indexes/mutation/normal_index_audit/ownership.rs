//! Fresh catalog-wide original tree-page qualification; no staged page authority.
use super::{tree::Tree, vector};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, NativeIndex, TableCheck, TableSchema,
    WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        data_type::{CURRENCY, INT64, SHORT_BINARY},
        definition,
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::mutation::map_authority::{self, Owner},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) struct Ownership {
    address: usize,
    length: usize,
    definition: u32,
    trees: Vec<(u32, Tree)>,
}

fn protect(
    protected: &mut Vec<u32>,
    page: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    reader.tick()?;
    reader.work(protected.len())?;
    if !protected.contains(&page) {
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)?;
        protected
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        protected.push(page);
    }
    Ok(())
}

fn members(
    protected: &mut Vec<u32>,
    reference: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), InspectError> {
    for page in allocation::pages(reader, reference)? {
        protect(protected, page, reader)?;
    }
    Ok(())
}

impl Ownership {
    pub(in crate::table) fn capture(
        image: &NativeImage,
        checked: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_mode(image, checked, false, false, budget, cancelled)
    }

    /// Real action callers resolve native key numbers independently of dense
    /// UPDATE vectors. All physical alias and catalog/LVAL checks stay shared.
    #[cfg(feature = "journal")]
    pub(in crate::table) fn capture_actions(
        image: &NativeImage,
        checked: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_mode(image, checked, true, false, budget, cancelled)
    }

    /// INSERT additionally qualifies bounded Binary root leaves. This does not
    /// broaden numeric DELETE or any UPDATE target policy.
    #[cfg(feature = "journal")]
    pub(in crate::table) fn capture_insert(
        image: &NativeImage,
        checked: &TableCheck,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_mode(image, checked, true, true, budget, cancelled)
    }

    fn capture_mode(
        image: &NativeImage,
        checked: &TableCheck,
        actions: bool,
        binary: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let schema = checked.schema();
        let selected = schema.definition_page();
        // No orphan physical descriptor or incompatible logical alias may hide
        // another mutable owner, including when the operation changes no keys.
        for number in 0..schema.layout.physical_indexes {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, checked.indexes().len() as u64)
                .map_err(InspectError::from)?;
            if !checked
                .indexes()
                .iter()
                .any(|check| check.index().data_number() as usize == number)
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
        let mut owners = vector(checked.indexes().len(), budget, cancelled)?;
        let mut trees: Vec<(u32, Tree)> = vector(checked.indexes().len(), budget, cancelled)?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        for (position, check) in checked.indexes().iter().enumerate() {
            reader.tick()?;
            let index = check.index();
            let constraints = index
                .constraints()
                .ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))?;
            if check.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || constraints.ignore_nulls()
                || index.columns().is_empty()
                || index.data_number() as usize >= schema.layout.physical_indexes
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            reader.work(position)?;
            if let Some(prior) = checked.indexes()[..position]
                .iter()
                .map(|check| check.index())
                .find(|prior| prior.data_number() == index.data_number())
            {
                if prior.root_page() != index.root_page()
                    || prior.usage_map() != index.usage_map()
                    || prior.columns() != index.columns()
                    || prior.flags() != index.flags()
                {
                    return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                }
                continue;
            }
            let map = RepresentedMap::read(&mut reader, index.usage_map(), MapRole::Owned)?;
            map.require_allocated_storage(&mut reader, &global)?;
            // Retain the old dense discriminator and charged UPDATE trace.
            // Actions explicitly resolve actual native numbers, never stored IDs.
            let typed = if actions {
                reader.work(schema.columns().len())?;
                index.columns().first().and_then(|key| {
                    schema
                        .columns()
                        .iter()
                        .find(|field| field.number() == key.number())
                        .filter(|field| {
                            matches!(field.native_type(), INT64 | CURRENCY)
                                || binary && field.native_type() == SHORT_BINARY
                        })
                })
            } else {
                index.columns().first().and_then(|key| {
                    schema.columns().get(key.number()).filter(|field| {
                        field.number() == key.number()
                            && matches!(field.native_type(), INT64 | CURRENCY)
                    })
                })
            };
            let tree = if let Some(field) = typed {
                if field.native_type() == SHORT_BINARY {
                    #[cfg(feature = "journal")]
                    {
                        Tree::capture_binary(
                            image,
                            index,
                            selected,
                            field,
                            reader.budget,
                            cancelled,
                        )?
                    }
                    #[cfg(not(feature = "journal"))]
                    {
                        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
                    }
                } else {
                    Tree::capture_integral(image, index, selected, field, reader.budget, cancelled)?
                }
            } else {
                Tree::capture(image, index, selected, reader.budget, cancelled)?
            };
            for page in &tree.pages {
                reader.tick()?;
                global.require_allocated(&mut reader, *page)?;
                for (_, prior) in &trees {
                    reader.work(prior.pages.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                    if prior.pages.binary_search(page).is_ok() {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                }
            }
            owners.push(Owner::IndexOwned {
                definition: selected,
                physical: index.data_number(),
            });
            trees.push((index.data_number(), tree));
        }
        let mut protected = Vec::new();
        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            cancelled,
            |local, indexes, declarations, reader| {
                protect(&mut protected, 0, reader)?;
                definition::read_with_pages(reader, local.definition_page(), |page, reader| {
                    protect(&mut protected, page, reader)
                })?;
                members(&mut protected, local.layout.usage, reader)?;
                if local.definition_page() != selected {
                    for index in indexes {
                        members(&mut protected, index.usage_map(), reader)?;
                    }
                }
                for declaration in &declarations.entries {
                    members(&mut protected, declaration.references.owned(), reader)?;
                }
                Ok(())
            },
        )?;
        census.require_exclusive(&owners, reader.budget, cancelled)?;
        census.protect_storage(&mut protected, &mut reader)?;
        for (_, tree) in &trees {
            for page in &tree.pages {
                reader.tick()?;
                reader.work(protected.len())?;
                if protected.contains(page) {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
        }
        check(reader.budget, cancelled)?;
        Ok(Self {
            address: image.as_bytes().as_ptr() as usize,
            length: image.as_bytes().len(),
            definition: selected,
            trees,
        })
    }

    pub(in crate::table) fn require<'a>(
        &'a self,
        image: &NativeImage,
        schema: &TableSchema,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&'a Tree, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.trees.len() as u64)
            .map_err(InspectError::from)?;
        if self.address != image.as_bytes().as_ptr() as usize
            || self.length != image.as_bytes().len()
            || self.definition != schema.definition_page()
            || (index.root_page() as usize)
                .checked_mul(PAGE_BYTES)
                .and_then(|start| start.checked_add(PAGE_BYTES))
                .is_none_or(|end| end > self.length)
        {
            return Err(EditError::Preservation);
        }
        let (_, tree) = self
            .trees
            .iter()
            .find(|(physical, tree)| {
                *physical == index.data_number() && tree.root.number == index.root_page()
            })
            .ok_or(EditError::Preservation)?;
        Ok(tree)
    }
}
