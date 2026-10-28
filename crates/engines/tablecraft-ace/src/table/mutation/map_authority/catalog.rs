//! Inventory every local table before granting exclusivity to selected owners.
use super::{Census, Owner};
use crate::{
    InspectError, NativeImage, NativeIndex, NativeTableStorage, TableSchema,
    format::{
        allocation::GLOBAL_FREE_REFERENCE, bytes::u32_at, definition,
        layout::TABLE_FREE_MAP_OFFSET, reader::Reader,
    },
    header::check,
    inspect_table_schema,
    table::{
        indexes,
        long_value::maps as storage_maps,
        mutation::{EditError, WriteBlocker},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn capture_catalog(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    payload: impl FnMut(
        &TableSchema,
        &[NativeIndex],
        &storage_maps::Declarations,
        &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError>,
) -> Result<Census, EditError> {
    capture_catalog_for_initialization(image, None, budget, cancelled, payload)
}

pub(in crate::table) fn capture_catalog_for_initialization(
    image: &NativeImage,
    initializing: Option<(u32, &[usize])>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    mut payload: impl FnMut(
        &TableSchema,
        &[NativeIndex],
        &storage_maps::Declarations,
        &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError>,
) -> Result<Census, EditError> {
    let catalog = image.catalog();
    let mut census = Census::new();
    {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: catalog.header(),
            budget,
            cancelled,
            reads: 0,
        };
        census.capture(Owner::GlobalFree, GLOBAL_FREE_REFERENCE, &mut reader)?;
    }

    for origin in catalog.origins() {
        check(budget, cancelled)?;
        if origin.table_storage() != Some(NativeTableStorage::Local) {
            continue;
        }
        budget
            .charge(Resource::Steps, catalog.catalog().objects().len() as u64)
            .map_err(InspectError::from)?;
        let object = catalog
            .catalog()
            .objects()
            .iter()
            .find(|object| object.id() == origin.object_id())
            .ok_or(InspectError::Malformed)?;
        let schema = inspect_table_schema(
            &mut image.reader(),
            catalog.catalog().scope(),
            object.name().as_str(),
            budget,
            cancelled,
        )?;
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: catalog.header(),
            budget,
            cancelled,
            reads: 0,
        };
        let definition = schema.definition_page();
        census.capture(
            Owner::OrdinaryOwned { definition },
            schema.layout.usage,
            &mut reader,
        )?;
        let metadata = definition::read(&mut reader, definition)?;
        let hint = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        if hint != 0 {
            census.capture(Owner::OrdinaryCandidate { definition }, hint, &mut reader)?;
        }
        let indexes = indexes::definitions_with_tail(&mut reader, &schema)?;
        // Logical parsing does not expose orphan physical descriptors. Refuse
        // incomplete ownership coverage rather than silently omit their maps.
        for physical in 0..schema.layout.physical_indexes {
            reader.tick()?;
            let physical = u32::try_from(physical).map_err(|_| InspectError::Malformed)?;
            reader.work(indexes.indexes.len())?;
            if !indexes
                .indexes
                .iter()
                .any(|index| index.data_number() == physical)
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        for index in &indexes.indexes {
            census.capture(
                Owner::IndexOwned {
                    definition,
                    physical: index.data_number(),
                },
                index.usage_map(),
                &mut reader,
            )?;
        }
        let selected = initializing.filter(|(number, _)| *number == definition);
        let declarations = if let Some((_, columns)) = selected {
            storage_maps::declarations_for_initialization(&mut reader, &schema, columns)?
        } else {
            storage_maps::declarations(&mut reader, &schema)?
        };
        if !declarations.complete {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for declaration in &declarations.entries {
            reader.tick()?;
            if declaration.references.owned() == 0 && declaration.references.candidate() == 0 {
                let Some((_, columns)) = selected else {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                };
                reader.work(columns.len())?;
                if !columns.contains(&declaration.number) {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                continue;
            }
            census.capture(
                Owner::ColumnOwned {
                    definition,
                    column: declaration.number,
                },
                declaration.references.owned(),
                &mut reader,
            )?;
            census.capture(
                Owner::ColumnCandidate {
                    definition,
                    column: declaration.number,
                },
                declaration.references.candidate(),
                &mut reader,
            )?;
        }
        payload(&schema, &indexes.indexes, &declarations, &mut reader)?;
    }

    // No row payload is scanned by this collector. The visitor is the existing
    // LVAL consumer's separate payload-preservation census.
    Ok(census)
}
