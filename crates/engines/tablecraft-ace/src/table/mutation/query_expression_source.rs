//! Shared native target-carrier/system-schema validation, without write policy.
//! Both callers retain their own count, page, topology and placement admission.
use crate::{
    InspectError, NativeImage, NativeObjectOrigin, NativeTableStorage, TableSchema,
    format::{
        bytes::u32_at,
        data_type,
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        reader::{Reader, row_range},
        row::{MASK_BITS, OFFSET_MASK},
    },
    header::check,
    inspect_table_schema,
    table::record_layout,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::ObjectKind;
use thiserror::Error;

const CATALOG_COLUMNS: usize = 17;
const QUERY_COLUMNS: usize = 8;
const QUERY_VARIABLES: usize = 4;
const NATIVE_QUERY_TYPE: i16 = 5;
const NATIVE_CONTAINER_TYPE: i16 = 3;
const SYSTEM_COLUMN_FLAG: u8 = 0x10;
const SYSTEM_OBJECT_FLAGS: u32 = 0x8000_0002;

#[derive(Debug, Error)]
pub(super) enum SourceError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("native query catalog carrier or system schema is unsupported")]
    Source,
}

pub(super) struct Qualified {
    pub catalog: TableSchema,
    pub queries: TableSchema,
}

/// Preserve V1's check order: row-count policy runs in the schema condition,
/// before system-origin and column-flags checks. No action/profile is inferred.
pub(super) fn capture(
    image: &NativeImage,
    origin: &NativeObjectOrigin,
    admit_rows: impl FnOnce(&TableSchema) -> bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, SourceError> {
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let tables = image
        .catalog()
        .catalog()
        .lookup("Tables", Some(ObjectKind::Container), None)
        .map_err(InspectError::from)?
        .ok_or(SourceError::Source)?;
    let parent = image
        .catalog()
        .origins()
        .iter()
        .find(|candidate| candidate.object_id() == tables.id())
        .ok_or(SourceError::Source)?;
    if origin.type_code() != NATIVE_QUERY_TYPE
        || origin.parent_id() != parent.native_id()
        || parent.type_code() != NATIVE_CONTAINER_TYPE
        || !parent.is_system()
        || parent.flags() & !SYSTEM_OBJECT_FLAGS != 0
    {
        return Err(SourceError::Source);
    }
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    if catalog.columns().len() != CATALOG_COLUMNS || catalog.layout.max_columns != CATALOG_COLUMNS {
        return Err(SourceError::Source);
    }
    {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let page = reader.page(origin.page())?;
        let (flags, start, end) = row_range(&page, usize::from(origin.slot()))?;
        if flags & !OFFSET_MASK != 0
            || page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != catalog.definition_page()
        {
            return Err(SourceError::Source);
        }
        let row = &page[start..end];
        let framing = record_layout::parse(row, &catalog, reader.budget)?;
        if framing.count != CATALOG_COLUMNS
            || framing.offsets.len() != catalog.layout.variable_slots + 1
        {
            return Err(SourceError::Source);
        }
        // Required carrier names must be represented, not inferred absent from
        // a shortened/unknown catalog schema. All variable payloads except Name
        // refuse, including present-empty caches, properties and owner data.
        for (name, kind, fixed) in [
            ("Id", data_type::INT32, true),
            ("ParentId", data_type::INT32, true),
            ("Name", data_type::SHORT_TEXT, false),
            ("Type", data_type::INT16, true),
            ("DateCreate", data_type::DATE_TIME, true),
            ("DateUpdate", data_type::DATE_TIME, true),
            ("Owner", data_type::SHORT_BINARY, false),
            ("Flags", data_type::INT32, true),
            ("Database", data_type::LONG_TEXT, false),
            ("Connect", data_type::LONG_TEXT, false),
            ("ForeignName", data_type::SHORT_TEXT, false),
            ("RmtInfoShort", data_type::SHORT_BINARY, false),
            ("RmtInfoLong", data_type::OLE, false),
            ("Lv", data_type::OLE, false),
            ("LvProp", data_type::OLE, false),
            ("LvModule", data_type::OLE, false),
            ("LvExtra", data_type::OLE, false),
        ] {
            reader.tick()?;
            if !catalog.columns().iter().any(|column| {
                column.name() == name && column.native_type() == kind && column.fixed() == fixed
            }) {
                return Err(SourceError::Source);
            }
        }
        for column in catalog.columns() {
            reader.tick()?;
            let field = &column.field;
            if !field.fixed
                && !field.name.eq_ignore_ascii_case("Name")
                && row[framing.mask_start + field.number / MASK_BITS]
                    & (1 << (field.number % MASK_BITS))
                    != 0
            {
                return Err(SourceError::Source);
            }
        }
    }
    let schema =
        inspect_table_schema(&mut image.reader(), scope, "MSysQueries", budget, cancelled)?;
    if schema.columns().len() != QUERY_COLUMNS
        || schema.layout.max_columns != QUERY_COLUMNS
        || schema.layout.variable_slots != QUERY_VARIABLES
        || !admit_rows(&schema)
    {
        return Err(SourceError::Source);
    }
    for table in [&catalog, &schema] {
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == table.object_id())
            .ok_or(SourceError::Source)?;
        if !origin.is_system()
            || origin.flags() & !SYSTEM_OBJECT_FLAGS != 0
            || origin.parent_id() != parent.native_id()
            || origin.table_storage() != Some(NativeTableStorage::Local)
            || u32::try_from(origin.native_id()).ok() != Some(table.definition_page())
        {
            return Err(SourceError::Source);
        }
    }
    for column in schema.columns() {
        check(budget, cancelled)?;
        let flags = if column.fixed() { 3 } else { 2 };
        if column.flags() & !SYSTEM_COLUMN_FLAG != flags || column.extended_flags() != 0 {
            return Err(SourceError::Source);
        }
    }
    Ok(Qualified {
        catalog,
        queries: schema,
    })
}
