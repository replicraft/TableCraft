//! A represented GLOBAL/owned frontier does not grant candidate-map growth.
use super::{growing::model as growing, model::Family, qualify};
use crate::{
    InspectError, NativeImage,
    format::{
        allocation::{MapRole, RepresentedMap},
        reader::Reader,
    },
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;
const FRONTIER: u32 = 512;

pub(in crate::table::mutation) fn source() -> Vec<u8> {
    let case = growing::case(6, Family::Legacy, false, growing::Destination::EndOfFile);
    let mut before = case.before;
    before.resize(FRONTIER as usize * PAGE, 0);

    let mut global = vec![0; 69];
    global[0] = 1;
    raw::put32(&mut global, 1, 18);
    let mut page = raw::data_page(1, &[(0, global)]);
    raw::put16(&mut page, 2, (PAGE - 14 - 2 - 69) as u16);
    before[PAGE..2 * PAGE].copy_from_slice(&page);

    let mut global_bitmap = vec![0xff; PAGE];
    global_bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in 0..19 {
        global_bitmap[4 + number / 8] &= !(1 << (number % 8));
    }
    before[18 * PAGE..19 * PAGE].copy_from_slice(&global_bitmap);

    let mut owned_bitmap = vec![0; PAGE];
    owned_bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    owned_bitmap[4 + 12 / 8] |= 1 << (12 % 8);
    before[17 * PAGE..18 * PAGE].copy_from_slice(&owned_bitmap);

    let mut owned = vec![0; 69];
    owned[0] = 1;
    raw::put32(&mut owned, 1, 17);
    let mut candidate = vec![0; 69];
    candidate[5 + 12 / 8] |= 1 << (12 % 8);
    let mut page = raw::data_page(0, &[(0, owned), (0, candidate)]);
    raw::put16(&mut page, 2, (PAGE - 14 - 4 - 138) as u16);
    before[11 * PAGE..12 * PAGE].copy_from_slice(&page);
    before
}

#[test]
fn qualify_growing_mixed_represented_map_boundary_before_preview() {
    let rows = growing::case(6, Family::Legacy, false, growing::Destination::EndOfFile).rows;
    let before = source();
    qualify(&before, &rows, Family::Legacy);

    let mut budget = super::budget();
    let image = NativeImage::capture(
        &mut Cursor::new(&before),
        CatalogScope::new(1),
        &mut budget,
        &|| false,
    )
    .unwrap();
    let checked = gates::before_slot(&image, "Items", &mut budget, &|| false).unwrap();
    Metadata::capture_for_mixed_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut budget,
        &|| false,
    )
    .unwrap()
    .require_data_page(12, &mut budget, &|| false)
    .unwrap();

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, 1 << 8, MapRole::GlobalFree).unwrap();
    let owned = RepresentedMap::read(&mut reader, 11 << 8, MapRole::Owned).unwrap();
    let candidate =
        RepresentedMap::read(&mut reader, (11 << 8) | 1, MapRole::TableFreeSpace).unwrap();
    let free = global.future_cell(&mut reader, FRONTIER).unwrap();
    assert_ne!(free.before & free.mask, 0);
    let unused = owned.future_cell(&mut reader, FRONTIER).unwrap();
    assert_eq!(unused.before & unused.mask, 0);
    assert!(matches!(
        candidate.future_cell(&mut reader, FRONTIER),
        Err(InspectError::Unsupported)
    ));
}
