//! Raw append into existing packed DATA free space, preserving all prior slots.
use super::put16;

pub(super) fn append_record(page: &mut [u8], row: &[u8]) {
    assert_eq!(&page[..2], &[1, 1]);
    let count = u16::from_le_bytes(page[12..14].try_into().unwrap()) as usize;
    let free = u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
    let slot = 14 + 2 * count;
    let end = slot + free;
    let start = end.checked_sub(row.len()).unwrap();
    assert!(start >= slot + 2);
    assert!(page[slot..slot + 2].iter().all(|byte| *byte == 0));
    assert!(page[start..end].iter().all(|byte| *byte == 0));
    page[start..end].copy_from_slice(row);
    put16(page, slot, start as u16);
    put16(page, 12, (count + 1) as u16);
    put16(page, 2, (free - row.len() - 2) as u16);
}
