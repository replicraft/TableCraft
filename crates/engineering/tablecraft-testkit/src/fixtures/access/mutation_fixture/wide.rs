//! One complete original scalar row with an independently sized column census.
use crate::fixtures::access::{data_page, mutation_fixture};

pub fn database(count: usize) -> Vec<u8> {
    assert!((1..=255).contains(&count));
    let names = (0..count)
        .map(|index| format!("Field{index}"))
        .collect::<Vec<_>>();
    let declared_bytes = 63
        + 25 * count
        + names
            .iter()
            .map(|name| 2 + 2 * name.encode_utf16().count())
            .sum::<usize>();
    assert!(
        declared_bytes <= 4096,
        "this raw oracle has one definition page"
    );
    let fields = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), 4, (index * 4) as u16, 4))
        .collect::<Vec<_>>();
    let mut bytes = mutation_fixture::database();
    bytes[10 * 4096..11 * 4096].copy_from_slice(&mutation_fixture::definition(&fields, 1, 11 << 8));
    let mut row = (count as u16).to_le_bytes().to_vec();
    for index in 0..count {
        row.extend((index as i32 + 1).to_le_bytes());
    }
    let mut mask = vec![0; count.div_ceil(8)];
    for index in 0..count {
        mask[index / 8] |= 1 << (index % 8);
    }
    row.extend(mask);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &[(0, row)]));
    bytes
}
