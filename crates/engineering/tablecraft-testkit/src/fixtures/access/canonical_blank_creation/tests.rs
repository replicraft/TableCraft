//! Raw structural assertions independent of production readers and creators.
use super::source;

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn long(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn records(page: &[u8]) -> Vec<&[u8]> {
    let count = word(page, 12);
    let mut end = page.len();
    let mut rows = Vec::new();
    for slot in 0..count {
        let start = word(page, 14 + 2 * slot);
        assert!(start >= 14 + 2 * count && start <= end);
        rows.push(&page[start..end]);
        end = start;
    }
    assert_eq!(word(page, 2), end - 14 - 2 * count);
    rows
}

#[test]
fn blank_has_complete_system_catalog_and_distinct_empty_long_value_maps() {
    let bytes = source();
    assert_eq!(bytes.len(), 28_672);
    assert_eq!(&bytes[..21], b"\x00\x01\x00\x00Standard ACE DB\x00\x05");
    let (pages, remainder) = bytes.as_chunks::<4096>();
    assert!(remainder.is_empty());
    for (index, expected) in [
        (1, [1, 1]),
        (2, [2, 1]),
        (3, [1, 1]),
        (4, [1, 1]),
        (5, [2, 1]),
        (6, [2, 1]),
    ] {
        assert_eq!(&pages[index][..2], &expected);
    }
    assert_eq!(records(&pages[1]), vec![&[0_u8; 13][..]]);

    let maps = records(&pages[3]);
    assert_eq!(maps.len(), 22);
    for (slot, map) in maps.iter().enumerate() {
        let mut expected = [0_u8; 13];
        if matches!(slot, 0 | 4) {
            expected[5] = 0x10;
        }
        assert_eq!(*map, expected, "map cell {slot}");
    }

    let catalog = &pages[2];
    assert_eq!(long(catalog, 16), 7);
    assert_eq!((word(catalog, 41), word(catalog, 43)), (17, 11));
    let mut tail = 63 + 25 * 17;
    for _ in 0..17 {
        tail += 2 + word(catalog, tail);
    }
    let mut seen_maps = Vec::new();
    for (index, expected_field) in [8, 9, 12, 13, 14, 15, 16].into_iter().enumerate() {
        assert_eq!(word(catalog, tail), expected_field);
        for (offset, slot) in [(2, 8 + 2 * index), (6, 9 + 2 * index)] {
            let reference = long(catalog, tail + offset);
            assert_eq!(reference, 768 + slot as u32);
            assert!(!seen_maps.contains(&reference));
            seen_maps.push(reference);
            assert_eq!(maps[slot], [0_u8; 13]);
        }
        tail += 10;
    }
    assert_eq!(word(catalog, tail), 0xffff);

    let rows = records(&pages[4]);
    let expected = [
        (0x0f00_0000, 0, "DatabaseRoot", 3),
        (0x0f00_0001, 0x0f00_0000, "Tables", 3),
        (0x0f00_0002, 0x0f00_0000, "Databases", 3),
        (0x0f00_0003, 0x0f00_0000, "Relationships", 3),
        (2, 0x0f00_0001, "MSysObjects", 1),
        (5, 0x0f00_0001, "MSysRelationships", 1),
        (6, 0x0f00_0001, "MSysACEs", 1),
    ];
    assert_eq!(rows.len(), expected.len());
    for (row, (id, parent, name, kind)) in rows.iter().zip(expected) {
        assert_eq!(word(row, 0), 17);
        assert_eq!(long(row, 2), id);
        assert_eq!(long(row, 6), parent);
        assert_eq!(word(row, 10), kind);
        assert_eq!(long(row, 28), 0x8000_0002);
        assert_eq!(&row[row.len() - 3..], &[0xbf, 0, 0]);
        let name_end = row.len() - 29;
        let (pairs, remainder) = row[32..name_end].as_chunks::<2>();
        assert!(remainder.is_empty());
        let units = pairs
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(String::from_utf16(&units).unwrap(), name);
    }
    for (page, columns, owned, candidate) in [(5, 8, 769, 773), (6, 4, 770, 774)] {
        assert_eq!(long(&pages[page], 16), 0);
        assert_eq!(word(&pages[page], 41), columns);
        assert_eq!(long(&pages[page], 55), owned);
        assert_eq!(long(&pages[page], 59), candidate);
    }
}
