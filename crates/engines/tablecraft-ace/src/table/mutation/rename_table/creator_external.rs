//! The explicit OFF external body in the canonical single-user-table role layout.
mod field_properties;
use super::{TableRenameError, source};
use crate::{
    FileFormat, InspectError, NativeImage, NativeObjectOrigin, TableProperties, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        data_type,
        layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, TABLE_FREE_MAP_OFFSET, UPDATABLE_FLAG},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{OFFSET_MASK, ROW_COUNT_OFFSET, reference, slot_offset},
    },
    header::check,
    inspect_table_indexes, inspect_table_schema,
    table::{long_value::maps, mutation::lval_ownership::Inventory},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_DEFINITION: u32 = 2;
const MAP_PAGE: u32 = 3;
const CATALOG_DATA: u32 = 4;
const USER_DEFINITION: u32 = 7;
const USER_DATA: u32 = 8;
const USER_INDEX: u32 = 9;
const PRIMARY_INDEX_TYPE: u8 = 1;
const PRIMARY_INDEX_FLAGS: u8 = 0x89;
const USER_CATALOG_SLOT: u8 = 7;
const DATABASE_CATALOG_SLOT: u8 = 8;
const DATABASE_ID: i32 = 0x0f00_0004;
const DATABASE_PARENT: i32 = 0x0f00_0002;
const DATABASE_FLAGS: u32 = 0x8000_0000;
const PROPERTY_COLUMN: usize = 14;
const PROPERTY_VARIABLE: usize = 8;
const OWNED_SLOT: u8 = 16;
const CANDIDATE_SLOT: u8 = 17;
const USER_OWNED_SLOT: u8 = 3;
const USER_CANDIDATE_SLOT: u8 = 7;
const USER_INDEX_SLOT: u8 = 22;
const SINGLE_ROW: u32 = 0x4000_0000;
const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();
const PROPERTY_BYTES: usize = 152;
const MAX_CREATOR_COLUMNS: usize = 16;
const NO_INDEX_PAGE_COUNT: u32 = 10;
const SYSTEM_FLAGS: u32 = 0x8000_0002;
const SYSTEM_ROLES: [(i32, i32, i16, &str); 7] = [
    (0x0f00_0000, 0, 3, "DatabaseRoot"),
    (0x0f00_0001, 0x0f00_0000, 3, "Tables"),
    (0x0f00_0002, 0x0f00_0000, 3, "Databases"),
    (0x0f00_0003, 0x0f00_0000, 3, "Relationships"),
    (2, 0x0f00_0001, 1, "MSysObjects"),
    (5, 0x0f00_0001, 1, "MSysRelationships"),
    (6, 0x0f00_0001, 1, "MSysACEs"),
];

pub(super) struct Binding {
    page: u32,
    user_properties: TableProperties,
}

impl Binding {
    pub(super) fn capture(
        image: &NativeImage,
        catalog: &TableSchema,
        database: &NativeObjectOrigin,
        user: &NativeObjectOrigin,
        properties: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, TableRenameError> {
        check(budget, cancelled)?;
        if image.catalog().header().format() != FileFormat::Ace2016
            || catalog.definition_page() != CATALOG_DEFINITION
            || catalog.declared_rows() != u32::from(DATABASE_CATALOG_SLOT) + 1
            || database.native_id() != DATABASE_ID
            || database.parent_id() != DATABASE_PARENT
            || database.flags() != DATABASE_FLAGS
            || database.page() != CATALOG_DATA
            || database.slot() != DATABASE_CATALOG_SLOT
            || user.native_id() != USER_DEFINITION as i32
            || user.page() != CATALOG_DATA
            || user.slot() != USER_CATALOG_SLOT
            || properties.len() != PROPERTY_BYTES
        {
            return Err(TableRenameError::Source);
        }
        for (slot, (id, parent, kind, name)) in SYSTEM_ROLES.into_iter().enumerate() {
            check(budget, cancelled)?;
            let origin = image
                .catalog()
                .origins()
                .get(slot)
                .ok_or(TableRenameError::Source)?;
            let object = image
                .catalog()
                .catalog()
                .get(origin.object_id())
                .ok_or(TableRenameError::Source)?;
            budget
                .charge(
                    Resource::Steps,
                    name.len().max(object.name().as_str().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if origin.native_id() != id
                || origin.parent_id() != parent
                || origin.type_code() != kind
                || origin.flags() != SYSTEM_FLAGS
                || origin.page() != CATALOG_DATA
                || usize::from(origin.slot()) != slot
                || object.name().as_str() != name
            {
                return Err(TableRenameError::Source);
            }
        }
        let property = catalog
            .columns()
            .iter()
            .find(|column| column.number() == PROPERTY_COLUMN)
            .ok_or(TableRenameError::Source)?;
        if property.name() != "LvProp"
            || property.fixed()
            || property.field.variable != PROPERTY_VARIABLE
            || property.native_type() != data_type::OLE
        {
            return Err(TableRenameError::Source);
        }
        let scope = image.catalog().catalog().scope();
        let object = image
            .catalog()
            .catalog()
            .get(user.object_id())
            .ok_or(TableRenameError::Source)?;
        let schema = inspect_table_schema(
            &mut image.reader(),
            scope,
            object.name().as_str(),
            budget,
            cancelled,
        )?;
        source::closed_catalog_geometry(&schema, budget, cancelled)?;
        if schema.columns().is_empty()
            || schema.columns().len() > MAX_CREATOR_COLUMNS
            || schema.layout.physical_indexes > 1
            || schema.layout.logical_indexes != schema.layout.physical_indexes
            || schema.layout.usage != reference(MAP_PAGE, USER_OWNED_SLOT)
            || image.catalog().header().page_count()
                != NO_INDEX_PAGE_COUNT + schema.layout.physical_indexes as u32
        {
            return Err(TableRenameError::Source);
        }
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let flags = if column.fixed() {
                FIXED_FLAG | UPDATABLE_FLAG
            } else {
                UPDATABLE_FLAG
            };
            let generated = column.flags() == (FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG);
            if column.extended_flags() != 0
                || (column.flags() != flags
                    && !(generated
                        && position == 0
                        && column.native_type() == data_type::INT32
                        && schema.layout.physical_indexes == 1))
            {
                return Err(TableRenameError::Source);
            }
        }
        let page = image.catalog().header().page_count() - 1;
        let user_properties = field_properties::qualify(image, &schema, budget, cancelled)?;
        let binding = Self {
            page,
            user_properties,
        };
        let mut input = image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let definition = reader.page(USER_DEFINITION)?;
        if u32_at(&definition, TABLE_FREE_MAP_OFFSET)? != reference(MAP_PAGE, USER_CANDIDATE_SLOT)
            || allocation::pages(&mut reader, schema.layout.usage)? != [USER_DATA]
        {
            return Err(TableRenameError::Source);
        }
        if schema.layout.physical_indexes == 1 {
            let indexes = inspect_table_indexes(
                &mut image.reader(),
                scope,
                schema.name(),
                reader.budget,
                cancelled,
            )?;
            let [index] = indexes.indexes() else {
                return Err(TableRenameError::Source);
            };
            if index.native_type() != PRIMARY_INDEX_TYPE
                || index.flags() != PRIMARY_INDEX_FLAGS
                || index.root_page() != USER_INDEX
                || index.usage_map() != reference(MAP_PAGE, USER_INDEX_SLOT)
                || index.columns().len() != 1
                || index.columns()[0].descending()
            {
                return Err(TableRenameError::Source);
            }
        }
        let declarations = maps::declarations(&mut reader, catalog)?;
        if !declarations.complete {
            return Err(TableRenameError::Source);
        }
        let declaration = declarations
            .entries
            .iter()
            .find(|entry| entry.number == PROPERTY_COLUMN)
            .ok_or(TableRenameError::Source)?;
        if declaration.references.owned() != reference(MAP_PAGE, OWNED_SLOT)
            || declaration.references.candidate() != reference(MAP_PAGE, CANDIDATE_SLOT)
        {
            return Err(TableRenameError::Source);
        }
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated(&mut reader, page)?;
        binding.body(&mut reader, properties)?;

        // One shared census includes every real DATA, INDEX, TDEF, map and live
        // LVAL role. This is not authority inferred from a constructor marker.
        let inventory = Inventory::capture(image, reader.budget, cancelled)
            .map_err(source::serialization_error)?;
        inventory
            .require_exclusive(catalog, PROPERTY_COLUMN, &[page], reader.budget, cancelled)
            .map_err(source::serialization_error)?;
        check(reader.budget, cancelled)?;
        Ok(binding)
    }

    pub(super) fn user_properties(&self) -> Option<&[u8]> {
        let raw = self.user_properties.raw_bytes();
        (!raw.is_empty()).then_some(raw)
    }

    pub(super) fn descriptor(
        &self,
        value: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), TableRenameError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, value.len() as u64)
            .map_err(InspectError::from)?;
        if value.len() != DESCRIPTOR_BYTES
            || u32_at(value, 0)? != SINGLE_ROW | PROPERTY_BYTES as u32
            || u32_at(value, size_of::<u32>())? != reference(self.page, 0)
            || u32_at(value, 2 * size_of::<u32>())? != 0
        {
            return Err(TableRenameError::Dependency);
        }
        Ok(())
    }

    pub(super) fn members(
        &self,
        definition: u32,
        column: usize,
        reference_value: u32,
        members: &[u32],
    ) -> Result<(), TableRenameError> {
        if definition == CATALOG_DEFINITION && column == PROPERTY_COLUMN {
            if ![
                reference(MAP_PAGE, OWNED_SLOT),
                reference(MAP_PAGE, CANDIDATE_SLOT),
            ]
            .contains(&reference_value)
                || members != [self.page]
            {
                return Err(TableRenameError::Source);
            }
        } else if !members.is_empty() {
            return Err(TableRenameError::Source);
        }
        Ok(())
    }

    fn body<R: crate::NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        properties: &[u8],
    ) -> Result<(), TableRenameError> {
        let page = reader.page(self.page)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || page[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()] != *b"LVAL"
            || page[OWNER_OFFSET + size_of::<u32>()..ROW_COUNT_OFFSET] != [0; size_of::<u32>()]
            || row_count(&page)? != 1
        {
            return Err(TableRenameError::Source);
        }
        let (raw, start, end) = row_range(&page, 0)?;
        if raw & !OFFSET_MASK != 0
            || end != PAGE_BYTES
            || end - start != PROPERTY_BYTES
            || page[start..end] != *properties
            || usize::from(u16_at(&page, DATA_SIGNATURE.len())?) != start - slot_offset(1)
            || page[slot_offset(1)..start].iter().any(|byte| *byte != 0)
        {
            return Err(TableRenameError::Source);
        }
        Ok(())
    }
}
