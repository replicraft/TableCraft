//! Actual column-owned pages, independently of any object's current descriptor.
use super::{catalog, column, maps};
use crate::{
    EditError, NativeImage,
    format::reader::Reader,
    table::mutation::owned_lval::maps::{EofAllocation, Maps},
};
use tablecraft_core::limits::ResourceBudget;

/// Observe allocated LvProp column storage without applying writer-page policy.
/// Empty user LvProp does not imply that MSysDb/another object owns no rows.
pub(crate) fn pages(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u32>, EditError> {
    let schema = catalog(image, budget, cancelled)?;
    let selected = column(&schema)?;
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let references = maps::references(&mut reader, &schema, selected)?;
    let Maps { long_value, .. } = Maps::capture_from_reader(
        &mut reader,
        &schema,
        references,
        EofAllocation::RequireAllocated,
    )?;
    Ok(long_value.pages)
}
