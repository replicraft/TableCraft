//! Read only the actual selected catalog field carrier; never mutation admission.
use super::{EditError, catalog, column};
use crate::{InspectError, NativeImage, TableSchema, format::reader::Reader, header::check};
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
    let descriptor = super::descriptor::capture(&mut reader, &catalog, column, origin)?;
    let external = descriptor.external()?;
    check(reader.budget, cancelled)?;
    Ok(external)
}
