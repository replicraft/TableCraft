//! Read only the actual selected catalog field carrier; never mutation admission.
use super::{EditError, catalog, column};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{data_type::OLE, reader::Reader},
    header::check,
    table::{
        long_value::trace::{self, Kind},
        record_layout,
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) fn external(
    image: &NativeImage,
    table: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    check(budget, cancelled)?;
    let catalog = catalog(image, budget, cancelled)?;
    let column = column(&catalog)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == table.object_id())
        .ok_or(InspectError::Malformed)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let page = reader.page(origin.page())?;
    let record = reader.row_location(
        origin.page(),
        origin.slot(),
        catalog.definition_page(),
        &page,
    )?;
    let framed = record_layout::parse(&record.data, &catalog, reader.budget)?;
    let field = catalog.columns()[column].physical_field();
    if field.fixed
        || catalog.columns()[column].native_type() != OLE
        || framed.count != catalog.layout.max_columns
        || framed.offsets.len() != catalog.layout.variable_slots + 1
        || field.number >= framed.count
        || field.variable >= catalog.layout.variable_slots
    {
        return Err(InspectError::Unsupported.into());
    }
    let mask = record
        .data
        .get(framed.mask_start + field.number / u8::BITS as usize)
        .ok_or(InspectError::Malformed)?;
    if mask & (1 << (field.number % u8::BITS as usize)) == 0 {
        check(reader.budget, cancelled)?;
        return Ok(false);
    }
    let descriptor = record
        .data
        .get(framed.offsets[field.variable]..framed.offsets[field.variable + 1])
        .ok_or(InspectError::Malformed)?;
    reader.work(descriptor.len())?;
    let external = matches!(trace::kind(descriptor)?, Kind::Single | Kind::Chained);
    check(reader.budget, cancelled)?;
    Ok(external)
}
