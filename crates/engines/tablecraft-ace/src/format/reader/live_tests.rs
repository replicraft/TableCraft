use super::{live_row_range, row_range};
use crate::InspectError;

#[test]
fn logical_enumeration_skips_empty_deleted_hidden_slots_but_checks_live_ranges() {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[1, 1]);
    page[12..14].copy_from_slice(&2_u16.to_le_bytes());
    page[14..16].copy_from_slice(&4090_u16.to_le_bytes());
    page[16..18].copy_from_slice(&0xcffa_u16.to_le_bytes());
    assert_eq!(live_row_range(&page, 0), Ok(Some((4090, 4090, 4096))));
    assert_eq!(row_range(&page, 1), Err(InspectError::Malformed));
    assert_eq!(live_row_range(&page, 1), Ok(None));
    assert_eq!(live_row_range(&page, 2), Err(InspectError::Malformed));

    page[16..18].copy_from_slice(&4090_u16.to_le_bytes());
    assert_eq!(live_row_range(&page, 1), Err(InspectError::Malformed));
    page[16..18].copy_from_slice(&0x4ffa_u16.to_le_bytes());
    assert_eq!(live_row_range(&page, 1), Err(InspectError::Malformed));
    page[16..18].copy_from_slice(&0x2ffa_u16.to_le_bytes());
    assert_eq!(live_row_range(&page, 1), Err(InspectError::Unsupported));
}
