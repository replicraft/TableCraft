//! Actual prior independent page/placement owners, with new row/key inputs.
use super::{Family, PAGE, Row, frames, keys};
use tablecraft_testkit::access as raw;

pub(super) fn original(
    rows: &[Row],
    records: &[Vec<u8>],
    family: Family,
    descending: bool,
    forwarded: bool,
) -> Vec<u8> {
    let eof = if forwarded { 18 } else { 17 };
    let mut bytes = super::base::database();
    bytes.resize(eof * PAGE, 0);
    for (page, end) in [(2, 398), (7, 449)] {
        raw::put16(&mut bytes, page * PAGE + end, 0xffff);
    }
    bytes[10 * PAGE..11 * PAGE].copy_from_slice(&super::schema::definition(
        rows.len(),
        family,
        descending,
    ));
    let mut physical = records
        .iter()
        .cloned()
        .map(|bytes| (0, bytes))
        .collect::<Vec<_>>();
    let mut owned = vec![12];
    if forwarded {
        physical[0].0 = 0x4000;
        physical[0].1.fill(0x5d);
        physical[0].1[..4].copy_from_slice(&(17_u32 << 8).to_le_bytes());
        owned.push(17);
        bytes[17 * PAGE..18 * PAGE]
            .copy_from_slice(&frames::data::data_page(&[(0x8000, records[0].clone())]));
    }
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&frames::data::data_page(&physical));
    bytes[11 * PAGE..12 * PAGE]
        .copy_from_slice(&frames::data::map_page(0, &[owned.clone(), owned]));
    bytes[14 * PAGE..15 * PAGE].copy_from_slice(&frames::data::map_page(0, &[vec![13], vec![15]]));
    bytes[16 * PAGE..17 * PAGE].fill(0xcc);
    bytes[PAGE..2 * PAGE]
        .copy_from_slice(&frames::data::map_page(1, &[(eof as u32..512).collect()]));

    let mut seed = vec![0xa7; PAGE];
    seed[..480].fill(0);
    for (physical, number) in [13, 15].into_iter().enumerate() {
        let entries = keys::entries(rows, family, descending, physical);
        bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(&frames::index::index_page(
            Some(&seed),
            &entries,
            (0, 0),
            0,
            false,
        ));
    }
    bytes
}

pub(super) fn fitted(
    before: &[u8],
    original_records: &[Vec<u8>],
    records: &[Vec<u8>],
    forwarded: bool,
) -> Vec<u8> {
    let mut after = before.to_vec();
    for (ordinal, record) in records.iter().enumerate() {
        assert_eq!(record.len(), original_records[ordinal].len());
        let (page, slot) = if forwarded && ordinal == 0 {
            (17, 0)
        } else {
            (12, ordinal)
        };
        let at = page * PAGE + 14 + slot * 2;
        let start =
            usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()) & 0x1fff);
        after[page * PAGE + start..page * PAGE + start + record.len()].copy_from_slice(record);
    }
    after
}

pub(super) fn growing(
    before: &[u8],
    original_records: &[Vec<u8>],
    records: &[Vec<u8>],
) -> (Vec<u8>, Vec<u32>) {
    let mut after = before.to_vec();
    let mut candidate = 12;
    let mut owned = vec![12];
    let mut hints = owned.clone();
    for (ordinal, record) in records.iter().enumerate() {
        if record.len() == original_records[ordinal].len() {
            let at = 12 * PAGE + 14 + ordinal * 2;
            let start = usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
            after[12 * PAGE + start..12 * PAGE + start + record.len()].copy_from_slice(record);
        } else {
            frames::data::append_record(
                &mut after,
                ordinal,
                record,
                &mut candidate,
                &mut owned,
                &mut hints,
            );
        }
    }
    after[11 * PAGE..12 * PAGE]
        .copy_from_slice(&frames::data::map_page(0, &[owned.clone(), hints]));
    let pages = owned.into_iter().filter(|number| *number != 12).collect();
    (after, pages)
}

pub(super) fn indexes(
    after: &mut Vec<u8>,
    before: &[u8],
    rows: &[Row],
    family: Family,
    descending: bool,
    split: bool,
) -> Vec<u32> {
    let mut added = Vec::new();
    for (physical, number) in [13, 15].into_iter().enumerate() {
        let entries = keys::entries(rows, family, descending, physical);
        let original = &before[number * PAGE..(number + 1) * PAGE];
        if split {
            let right = (after.len() / PAGE) as u32;
            let left = right + 1;
            let middle = entries.len() / 2;
            after.extend(frames::index::index_page(
                None,
                &entries[middle..],
                (left, 0),
                0,
                false,
            ));
            after.extend(frames::index::index_page(
                None,
                &entries[..middle],
                (0, right),
                0,
                false,
            ));
            let separator = frames::index::separator(&entries[middle - 1], left);
            let root = frames::index::index_page(Some(original), &[separator], (0, 0), right, true);
            after[number * PAGE..(number + 1) * PAGE].copy_from_slice(&root);
            added.extend([right, left]);
        } else {
            after[number * PAGE..(number + 1) * PAGE].copy_from_slice(&frames::index::index_page(
                Some(original),
                &entries,
                (0, 0),
                0,
                false,
            ));
        }
    }
    if split {
        after[14 * PAGE..15 * PAGE].copy_from_slice(&frames::data::map_page(
            0,
            &[vec![13, added[0], added[1]], vec![15, added[2], added[3]]],
        ));
    }
    let end = (after.len() / PAGE) as u32;
    after[PAGE..2 * PAGE].copy_from_slice(&frames::data::map_page(1, &[(end..512).collect()]));
    added
}
