//! Selected query payload storage and complete native role exclusion.
use super::{EditError, WriteBlocker, lval_ownership::Inventory, map_authority};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, NativeObjectOrigin, NativeTableStorage,
    QueryExpressionError,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        data_type, definition,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{MASK_BITS, OFFSET_MASK},
    },
    header::check,
    inspect_table_schema,
    table::{indexes::check_table_retained, record_layout},
};
use std::ops::Range;
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};
use tablecraft_model::ObjectKind;

const INLINE_HEADER_BYTES: usize = 3 * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const CATALOG_COLUMNS: usize = 17;
const QUERY_COLUMNS: usize = 8;
const QUERY_VARIABLES: usize = 4;
const QUERY_ROWS: u32 = 5;
const NATIVE_QUERY_TYPE: i16 = 5;
const NATIVE_CONTAINER_TYPE: i16 = 3;
const SYSTEM_COLUMN_FLAG: u8 = 0x10;
const SYSTEM_OBJECT_FLAGS: u32 = 0x8000_0002;

pub(crate) fn qualify(
    image: &NativeImage,
    origin: &NativeObjectOrigin,
    expected: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Range<usize>, QueryExpressionError> {
    check(budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let tables = image
        .catalog()
        .catalog()
        .lookup("Tables", Some(ObjectKind::Container), None)
        .map_err(InspectError::from)?
        .ok_or(QueryExpressionError::Source)?;
    let parent = image
        .catalog()
        .origins()
        .iter()
        .find(|candidate| candidate.object_id() == tables.id())
        .ok_or(QueryExpressionError::Source)?;
    if origin.type_code() != NATIVE_QUERY_TYPE
        || origin.parent_id() != parent.native_id()
        || parent.type_code() != NATIVE_CONTAINER_TYPE
        || !parent.is_system()
        || parent.flags() & !SYSTEM_OBJECT_FLAGS != 0
    {
        return Err(QueryExpressionError::Source);
    }
    let catalog =
        inspect_table_schema(&mut image.reader(), scope, "MSysObjects", budget, cancelled)?;
    if catalog.columns().len() != CATALOG_COLUMNS || catalog.layout.max_columns != CATALOG_COLUMNS {
        return Err(QueryExpressionError::Source);
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
            return Err(QueryExpressionError::Source);
        }
        let row = &page[start..end];
        let framing = record_layout::parse(row, &catalog, reader.budget)?;
        if framing.count != CATALOG_COLUMNS
            || framing.offsets.len() != catalog.layout.variable_slots + 1
        {
            return Err(QueryExpressionError::Source);
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
                return Err(QueryExpressionError::Source);
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
                return Err(QueryExpressionError::Source);
            }
        }
    }
    let schema =
        inspect_table_schema(&mut image.reader(), scope, "MSysQueries", budget, cancelled)?;
    if schema.columns().len() != QUERY_COLUMNS
        || schema.layout.max_columns != QUERY_COLUMNS
        || schema.layout.variable_slots != QUERY_VARIABLES
        || schema.declared_rows() != QUERY_ROWS
    {
        return Err(QueryExpressionError::Source);
    }
    for table in [&catalog, &schema] {
        let origin = image
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.object_id() == table.object_id())
            .ok_or(QueryExpressionError::Source)?;
        if !origin.is_system()
            || origin.flags() & !SYSTEM_OBJECT_FLAGS != 0
            || origin.parent_id() != parent.native_id()
            || origin.table_storage() != Some(NativeTableStorage::Local)
            || u32::try_from(origin.native_id()).ok() != Some(table.definition_page())
        {
            return Err(QueryExpressionError::Source);
        }
    }
    for column in schema.columns() {
        check(budget, cancelled)?;
        let flags = if column.fixed() { 3 } else { 2 };
        if column.flags() & !SYSTEM_COLUMN_FLAG != flags || column.extended_flags() != 0 {
            return Err(QueryExpressionError::Source);
        }
    }
    let expression = schema
        .columns()
        .iter()
        .find(|column| column.name().eq_ignore_ascii_case("Expression"))
        .ok_or(QueryExpressionError::Source)?;
    if expression.fixed()
        || expression.native_type() != crate::format::data_type::LONG_TEXT
        || expression.field.extended_flags != 0
    {
        return Err(QueryExpressionError::Source);
    }
    let attribute = schema
        .columns()
        .iter()
        .find(|column| column.name().eq_ignore_ascii_case("Attribute"))
        .ok_or(QueryExpressionError::Source)?;
    let (span, number) = {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let pages = allocation::pages(&mut reader, schema.layout.usage)?;
        let [number] = pages.as_slice() else {
            return Err(QueryExpressionError::Source);
        };
        let number = *number;
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition_page()
            || row_count(&page)? != QUERY_ROWS as usize
        {
            return Err(QueryExpressionError::Source);
        }
        let mut selected = None;
        for slot in 0..QUERY_ROWS as usize {
            reader.tick()?;
            let (flags, start, end) = row_range(&page, slot)?;
            if flags & !OFFSET_MASK != 0 {
                return Err(QueryExpressionError::Source);
            }
            let row = &page[start..end];
            let framing = record_layout::parse(row, &schema, reader.budget)?;
            if framing.count != QUERY_COLUMNS || framing.offsets.len() != QUERY_VARIABLES + 1 {
                return Err(QueryExpressionError::Source);
            }
            if row.get(size_of::<u16>() + attribute.field.offset)
                != Some(&crate::QueryAttribute::Column.code())
            {
                continue;
            }
            let begin = framing.offsets[expression.field.variable];
            let end = framing.offsets[expression.field.variable + 1];
            let stored = &row[begin..end];
            let encoded_length = expected.len() * size_of::<u16>();
            if selected.is_some()
                || stored.len() != INLINE_HEADER_BYTES + encoded_length
                || u32_at(stored, 0)? != INLINE_TAG | encoded_length as u32
                || stored[size_of::<u32>()..INLINE_HEADER_BYTES]
                    .iter()
                    .any(|byte| *byte != 0)
                || !stored[INLINE_HEADER_BYTES..]
                    .as_chunks::<{ size_of::<u16>() }>()
                    .0
                    .iter()
                    .zip(expected.bytes())
                    .all(|(pair, byte)| *pair == u16::from(byte).to_le_bytes())
            {
                return Err(QueryExpressionError::Source);
            }
            let base = number as usize * PAGE_BYTES + start + begin + INLINE_HEADER_BYTES;
            selected = Some(base..base + encoded_length);
        }
        let span = selected.ok_or(QueryExpressionError::Source)?;
        let free = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        let cell = free.cell(&mut reader, number)?;
        if cell.before & cell.mask != 0 {
            return Err(QueryExpressionError::Source);
        }
        (span, number)
    };
    for table in ["MSysObjects", "MSysQueries"] {
        let checked = check_table_retained(&mut image.reader(), scope, table, budget, cancelled)?;
        if checked
            .indexes()
            .iter()
            .any(|index| index.status() != IndexCheckStatus::Checked)
        {
            return Err(QueryExpressionError::Source);
        }
    }
    require_storage_roles(
        image,
        schema.definition_page(),
        schema.object_id(),
        number,
        budget,
        cancelled,
    )
    .map_err(storage_error)?;
    Ok(span)
}

fn storage_error(error: EditError) -> QueryExpressionError {
    match error {
        EditError::Inspect(error) => QueryExpressionError::Inspect(error),
        _ => QueryExpressionError::Source,
    }
}

fn require_storage_roles(
    image: &NativeImage,
    selected_definition: u32,
    selected_object: ObjectId,
    selected_page: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let long_values = Inventory::capture(image, budget, cancelled)?;
    long_values.require_no_long_value_data_overlap(&[selected_page], budget, cancelled)?;
    let maps = map_authority::capture_catalog(
        image,
        budget,
        cancelled,
        |schema, indexes, declarations, reader| {
            definition::read_with_pages(reader, schema.definition_page(), |number, _| {
                if number == selected_page {
                    return Err(InspectError::Unsupported);
                }
                Ok(())
            })?;
            if schema.definition_page() == selected_definition
                && schema.object_id() != selected_object
            {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let pages = allocation::pages(reader, schema.layout.usage)?;
            reader.work(pages.len())?;
            if schema.definition_page() != selected_definition && pages.contains(&selected_page) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            for index in indexes {
                let pages = allocation::pages(reader, index.usage_map())?;
                reader.work(pages.len())?;
                if pages.contains(&selected_page) {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
            for declaration in &declarations.entries {
                for reference in [
                    declaration.references.owned(),
                    declaration.references.candidate(),
                ] {
                    let pages = allocation::pages(reader, reference)?;
                    reader.work(pages.len())?;
                    if pages.contains(&selected_page) {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                }
            }
            Ok(())
        },
    )?;
    let mut protected = Vec::new();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    maps.protect_storage(&mut protected, &mut reader)?;
    reader.work(protected.len())?;
    if protected.contains(&selected_page) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(())
}
