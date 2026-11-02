//! Independent raw-cell XOR reconstruction, not producer claim arrays.
use crate::{
    EditError, NativeImage, NativeIndex,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn reconstruct(
    image: &NativeImage,
    indexes: [&NativeIndex; 2],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut bytes = image.reader();
    let mut reader = Reader {
        source: &mut bytes,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let first = image.catalog().header().page_count();
    for (position, index) in indexes.into_iter().enumerate() {
        for number in [first + position as u32 * 2, first + position as u32 * 2 + 1] {
            for (reference, role, set) in [
                (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree, false),
                (index.usage_map(), MapRole::Owned, true),
            ] {
                reader.tick()?;
                let map = RepresentedMap::read(&mut reader, reference, role)?;
                let cell = map.future_cell(&mut reader, number)?;
                if cell.mask.count_ones() != 1 || (cell.before & cell.mask != 0) == set {
                    return Err(EditError::Preservation);
                }
                reader.work(1)?;
                let target = expected
                    .get_mut(cell.offset)
                    .ok_or(EditError::Preservation)?;
                if *target & cell.mask != cell.before & cell.mask {
                    return Err(EditError::Preservation);
                }
                *target ^= cell.mask;
            }
        }
    }
    check(reader.budget, cancelled)?;
    Ok(())
}
