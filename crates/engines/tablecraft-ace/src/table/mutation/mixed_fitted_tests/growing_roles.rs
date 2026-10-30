//! A reader-valid orphan INDEX ownership claim cannot authorize DATA writes.
use super::{growing, model::Family, qualify};
use crate::{
    EditError, NativeImage, WriteBlocker,
    table::mutation::{gates, variable_split::metadata::Metadata},
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

#[test]
fn qualify_mixed_growth_body_role_alias_source_before_preview() {
    let case = growing::case(6, Family::Legacy, false, growing::Destination::OriginalPage);
    let mut aliased = case.before.clone();
    let page = 14 * 4096;
    let row = usize::from(u16::from_le_bytes(
        aliased[page + 14..page + 16].try_into().unwrap(),
    ));
    let cell = page + row + 5 + 12 / 8;
    assert_eq!(aliased[cell] & (1 << (12 % 8)), 0);
    aliased[cell] |= 1 << (12 % 8);
    assert_eq!(
        aliased
            .iter()
            .zip(&case.before)
            .filter(|(left, right)| left != right)
            .count(),
        1,
    );

    for (bytes, shared_body) in [(&case.before, false), (&aliased, true)] {
        qualify(bytes, &case.rows, case.family);
        let mut budget = super::budget();
        let image = NativeImage::capture(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            &mut budget,
            &|| false,
        )
        .unwrap();
        let checked = gates::before_slot(&image, "Items", &mut budget, &|| false).unwrap();
        let metadata = Metadata::capture_for_mixed_updates(
            &image,
            checked.schema(),
            checked.indexes(),
            &[1, 2],
            &mut budget,
            &|| false,
        )
        .unwrap();
        let result = metadata.require_data_page(12, &mut budget, &|| false);
        if shared_body {
            assert_eq!(result, Err(EditError::Unsupported(WriteBlocker::RowLayout)));
        } else {
            result.unwrap();
        }
    }
}
