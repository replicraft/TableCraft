//! Independent seven-page ACE 16 source with no ordinary user tables.
//!
//! These literal roles retain the existing native17 system declarations and
//! twenty-two map cells, including empty reserved user cells. No production
//! creator, imported database template or decoded metadata constructs the image.
use super::{canonical_creation as original, canonical_native17_creation as native17};

const PAGE_BYTES: usize = 4096;

/// Construct the complete unencrypted header-5 source from raw fixture laws.
pub fn source() -> Vec<u8> {
    let mut maps = vec![vec![0; 13]; 22];
    // Catalog owned and candidate cells both contain its sole DATA page.
    maps[0][5] = 0x10;
    maps[4][5] = 0x10;

    let objects = [
        (0x0f00_0000, 0, "DatabaseRoot", 3, 0x8000_0002),
        (0x0f00_0001, 0x0f00_0000, "Tables", 3, 0x8000_0002),
        (0x0f00_0002, 0x0f00_0000, "Databases", 3, 0x8000_0002),
        (0x0f00_0003, 0x0f00_0000, "Relationships", 3, 0x8000_0002),
        (2, 0x0f00_0001, "MSysObjects", 1, 0x8000_0002),
        (5, 0x0f00_0001, "MSysRelationships", 1, 0x8000_0002),
        (6, 0x0f00_0001, "MSysACEs", 1, 0x8000_0002),
    ];
    let catalog = objects
        .into_iter()
        .map(|(id, parent, name, kind, flags)| {
            let row = original::catalog_record(id, parent, name, kind, flags);
            native17::catalog_row(&row)
        })
        .collect();

    let pages = [
        original::header(),
        native17::packed(1, vec![vec![0; 13]]),
        native17::declaration(7),
        native17::packed(0, maps),
        native17::packed(2, catalog),
        original::definition(&original::RELATIONSHIPS, 0, 1),
        original::definition(&original::ACES, 0, 2),
    ];
    let bytes = pages.concat();
    assert_eq!(bytes.len(), 7 * PAGE_BYTES);
    bytes
}

#[cfg(test)]
mod tests;
