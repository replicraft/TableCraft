//! Independent raw DATA pages for complete-table and metadata traversal cases.
use super::{data_page, put32};

/// Replace a table's records with supplied raw rows on newly appended pages.
/// Literal slot framing and usage bits remain independent of production readers.
pub fn append_records(
    bytes: &mut Vec<u8>,
    definition: usize,
    usage: usize,
    records: Vec<(u16, Vec<u8>)>,
) {
    let count = records.len();
    let first_page = bytes.len() / 4096;
    let mut pages = Vec::new();
    let mut rows = Vec::new();
    let mut occupied = 14;
    for row in records {
        let cost = 2 + row.1.len();
        assert!(cost + 14 <= 4096);
        if occupied + cost > 4096 || rows.len() == 255 {
            pages.push(std::mem::take(&mut rows));
            occupied = 14;
        }
        occupied += cost;
        rows.push(row);
    }
    if !rows.is_empty() {
        pages.push(rows);
    }
    let end_page = first_page + pages.len();
    let mut bitmap = vec![0; 5 + end_page.div_ceil(8)];
    assert!(bitmap.len() + 16 <= 4096);
    for page in first_page..end_page {
        bitmap[5 + page / 8] |= 1 << (page % 8);
    }
    bytes.resize(end_page * 4096, 0);
    for (offset, rows) in pages.into_iter().enumerate() {
        let page = first_page + offset;
        bytes[page * 4096..(page + 1) * 4096].copy_from_slice(&data_page(definition as u32, &rows));
    }
    bytes[usage * 4096..(usage + 1) * 4096].copy_from_slice(&data_page(1, &[(0, bitmap)]));
    put32(bytes, definition * 4096 + 16, u32::try_from(count).unwrap());
}
