//! Selected query payload storage and complete native role exclusion.
use super::{EditError, WriteBlocker, lval_ownership::Inventory, map_authority};
use crate::{
    IndexCheckStatus, InspectError, NativeImage, NativeObjectOrigin, QueryExpressionError,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::OFFSET_MASK,
    },
    table::{indexes::check_table_retained, record_layout},
};
use std::ops::Range;
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

const INLINE_HEADER_BYTES: usize = 3 * size_of::<u32>();
const INLINE_TAG: u32 = 0x8000_0000;
const QUERY_COLUMNS: usize = 8;
const QUERY_VARIABLES: usize = 4;
const QUERY_ROWS: u32 = 5;

pub(crate) fn qualify(
    image: &NativeImage,
    origin: &NativeObjectOrigin,
    expected: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Range<usize>, QueryExpressionError> {
    let qualified = super::query_expression_source::capture(
        image,
        origin,
        |schema| schema.declared_rows() == QUERY_ROWS,
        budget,
        cancelled,
    )
    .map_err(|error| match error {
        super::query_expression_source::SourceError::Inspect(error) => {
            QueryExpressionError::Inspect(error)
        }
        super::query_expression_source::SourceError::Source => QueryExpressionError::Source,
    })?;
    let schema = qualified.queries;
    let _catalog = qualified.catalog;
    let scope = image.catalog().catalog().scope();
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
