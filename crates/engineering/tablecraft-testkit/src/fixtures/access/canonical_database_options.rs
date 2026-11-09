//! Independent explicit-OFF AFTER over unchanged canonical single-table originals.
use super::{database_properties, put16, put32};

const PAGE: usize = 4096;

/// Author the complete expected external-LVAL form without production codecs.
/// The caller supplies an original raw fixture, never a mutation candidate.
pub fn explicit_off(before: &[u8]) -> Vec<u8> {
    assert!(matches!(before.len(), 36864 | 40960));
    assert_eq!(
        u16::from_le_bytes(before[4 * PAGE + 12..4 * PAGE + 14].try_into().unwrap()),
        8
    );
    let new_page = before.len() / PAGE;
    let properties = database_properties::name_auto_correct_disabled();
    assert_eq!(properties.len(), 152);
    let mut after = before.to_vec();
    after.resize(before.len() + PAGE, 0);

    let row = database_row((new_page as u32) << 8);

    let catalog = 4 * PAGE;
    let low = usize::from(u16::from_le_bytes(
        before[catalog + 28..catalog + 30].try_into().unwrap(),
    ));
    let start = low - row.len();
    assert!(start >= 32);
    after[catalog + start..catalog + low].copy_from_slice(&row);
    put16(&mut after, catalog + 30, start as u16);
    put16(&mut after, catalog + 12, 9);
    put16(&mut after, catalog + 2, (start - 32) as u16);
    put32(&mut after, 2 * PAGE + 16, 9);
    for slot in [16, 17] {
        let at = 3 * PAGE + 14 + 2 * slot;
        let map_start = usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
        assert_eq!(
            &before[3 * PAGE + map_start..3 * PAGE + map_start + 13],
            &[0; 13]
        );
        after[3 * PAGE + map_start + 5 + new_page / 8] = 1 << (new_page % 8);
    }
    let lval = new_page * PAGE;
    after[lval..lval + 2].copy_from_slice(&[1, 1]);
    put16(&mut after, lval + 2, 3928);
    after[lval + 4..lval + 8].copy_from_slice(b"LVAL");
    put16(&mut after, lval + 12, 1);
    put16(&mut after, lval + 14, 3944);
    after[lval + 3944..lval + PAGE].copy_from_slice(&properties);
    after
}

/// Append the explicit OFF role to an already owned, fitting property page.
/// This independent fixture models the observed 11-page shared-column sources;
/// it performs no allocation, compaction or production codec operation.
pub fn append_explicit_off_to_owned_page(before: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let catalog = 4 * PAGE;
    let lval = 10 * PAGE;
    assert_eq!(&before[lval..lval + 2], &[1, 1]);
    assert_eq!(&before[lval + 4..lval + 8], b"LVAL");
    assert_eq!(
        u32::from_le_bytes(before[2 * PAGE + 16..2 * PAGE + 20].try_into().unwrap()),
        8
    );
    let catalog_count = word(before, catalog + 12);
    assert!(matches!(catalog_count, 8 | 9));
    let property_count = word(before, lval + 12);
    assert!((1..=255).contains(&property_count));
    let properties = database_properties::name_auto_correct_disabled();
    assert_eq!(properties.len(), 152);
    let reference = (10_u32 << 8) | property_count as u32;
    let row = database_row(reference);
    let catalog_low = word(before, catalog + 14 + 2 * (catalog_count - 1)) & 0x1fff;
    let catalog_start = catalog_low.checked_sub(row.len()).unwrap();
    let catalog_directory = 14 + 2 * (catalog_count + 1);
    assert!(catalog_start >= catalog_directory);
    let property_low = word(before, lval + 14 + 2 * (property_count - 1));
    let property_start = property_low.checked_sub(properties.len()).unwrap();
    let property_directory = 14 + 2 * (property_count + 1);
    assert!(property_start >= property_directory);
    assert!(
        before[catalog + catalog_start..catalog + catalog_low]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert!(
        before[lval + property_start..lval + property_low]
            .iter()
            .all(|byte| *byte == 0)
    );

    let mut after = before.to_vec();
    after[catalog + catalog_start..catalog + catalog_low].copy_from_slice(&row);
    put16(
        &mut after,
        catalog + 14 + 2 * catalog_count,
        catalog_start as u16,
    );
    put16(&mut after, catalog + 12, (catalog_count + 1) as u16);
    put16(
        &mut after,
        catalog + 2,
        (catalog_start - catalog_directory) as u16,
    );
    put32(&mut after, 2 * PAGE + 16, 9);
    after[lval + property_start..lval + property_low].copy_from_slice(&properties);
    put16(
        &mut after,
        lval + 14 + 2 * property_count,
        property_start as u16,
    );
    put16(&mut after, lval + 12, (property_count + 1) as u16);
    put16(
        &mut after,
        lval + 2,
        (property_start - property_directory) as u16,
    );
    after
}

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn database_row(reference: u32) -> Vec<u8> {
    let mut row = vec![0; 32];
    put16(&mut row, 0, 17);
    put32(&mut row, 2, 0x0f00_0004);
    put32(&mut row, 6, 0x0f00_0002);
    put16(&mut row, 10, 2);
    put32(&mut row, 28, 0x8000_0000);
    row.extend("MSysDb".encode_utf16().flat_map(u16::to_le_bytes));
    row.extend(0x4000_0098_u32.to_le_bytes());
    row.extend(reference.to_le_bytes());
    row.extend(0_u32.to_le_bytes());
    row.extend(56_u16.to_le_bytes());
    for variable in (0..11).rev() {
        let offset = if variable == 0 {
            32_u16
        } else if variable <= 8 {
            44
        } else {
            56
        };
        row.extend(offset.to_le_bytes());
    }
    row.extend(11_u16.to_le_bytes());
    row.extend([0x8f, 0x40, 0]);
    assert_eq!(row.len(), 85);

    row
}

#[cfg(test)]
mod tests;
