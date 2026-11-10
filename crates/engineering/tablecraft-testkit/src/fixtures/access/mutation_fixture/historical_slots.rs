//! Authored historical ordinal geometry, independent of native codecs/producers.
use super::{data_page, database as base, definition, put16};

/// Two active fields retain a native ordinal capacity of 301 after deletions.
/// One full-width record and one older record expose the same surviving schema.
pub fn database(profile: u8, variable: bool) -> Vec<u8> {
    let mut bytes = base();
    bytes[4..20].copy_from_slice(if profile == 1 {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    });
    bytes[20] = profile;
    let fields = if variable {
        [("Id", 4, 0, 4), ("Score", 10, 0, 100)]
    } else {
        [("Id", 4, 0, 4), ("Score", 4, 4, 4)]
    };
    let mut page = definition(&fields, 2, 11 << 8);
    put16(&mut page, 41, 301);
    put16(&mut page, 63 + 25 + 5, 300);
    if variable {
        put16(&mut page, 43, 300);
        put16(&mut page, 63 + 25 + 7, 299);
    }
    bytes[10 * 4096..11 * 4096].copy_from_slice(&page);

    let mut current = 301_u16.to_le_bytes().to_vec();
    current.extend(1_i32.to_le_bytes());
    if variable {
        current.extend([b'A', 0, b'B', 0]);
        // Slots 0..298 are retired and empty. Slot 299 owns UTF-16 "AB".
        for index in (0..=300).rev() {
            let offset = if index == 300 { 10_u16 } else { 6_u16 };
            current.extend(offset.to_le_bytes());
        }
        current.extend(300_u16.to_le_bytes());
    } else {
        current.extend(10_i32.to_le_bytes());
    }
    let mask = current.len();
    current.resize(mask + 38, 0);
    current[mask] = 1;
    current[mask + 37] = 0x10;

    let mut older = 1_u16.to_le_bytes().to_vec();
    older.extend(2_i32.to_le_bytes());
    older.push(1);
    let rows = data_page(10, &[(0, current), (0, older)]);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&rows);
    bytes
}
