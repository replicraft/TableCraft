//! Independent current-width Text INSERT bytes following a populated metadata ADD.
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
const DEFINITION: usize = 7;
const DATA: usize = 8;
const INDEX: usize = 9;
const DIRECTORY: usize = 14;
const ENTRIES: usize = 480;
const BITMAP: usize = 27;
const KEY_BYTES: usize = 9;

/// Append a generated ID, literal nullable Score and literal nullable Short Text.
/// The helper retains every historical record and authors native row/index bytes
/// independently of the engine's insertion and text codecs.
pub fn append_row(before: &[u8], score: Option<i32>, text: Option<&str>) -> Vec<u8> {
    fn word(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
    }

    assert_eq!(before.len(), 11 * PAGE);
    let definition = DEFINITION * PAGE;
    assert_eq!(
        &before[definition + 41..definition + 47],
        &[3, 0, 1, 0, 3, 0]
    );
    let column = definition + 75 + 2 * 25;
    assert_eq!(before[column], 10);
    assert_eq!(word(before, column + 7), 0);
    let mut payload = Vec::new();
    if let Some(text) = text {
        let units: Vec<_> = text.encode_utf16().collect();
        assert!(units.len() <= word(before, column + 23) / 2);
        if before[column + 16] & 1 != 0
            && units.len() > 2
            && units.iter().all(|unit| (1..=255).contains(unit))
        {
            payload.extend([255, 254]);
            payload.extend(units.iter().map(|unit| *unit as u8));
        } else {
            assert!(!text.starts_with('\u{feff}'));
            for unit in units {
                payload.extend(unit.to_le_bytes());
            }
        }
    }

    let data = DATA * PAGE;
    let count = word(before, data + 12);
    assert!((1..255).contains(&count));
    let end = word(before, data + DIRECTORY + (count - 1) * 2);
    assert_eq!(word(before, data + 2), end - DIRECTORY - count * 2);
    let id = u32::try_from(count + 1).unwrap();
    let mut row = vec![3, 0];
    row.extend(id.to_le_bytes());
    row.extend(score.unwrap_or(0).to_le_bytes());
    row.extend_from_slice(&payload);
    row.extend(u16::try_from(10 + payload.len()).unwrap().to_le_bytes());
    row.extend(10_u16.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(1 | if score.is_some() { 2 } else { 0 } | if text.is_some() { 4 } else { 0 });
    let start = end - row.len();
    assert!(start >= DIRECTORY + (count + 1) * 2);
    let mut after = before.to_vec();
    after[data + start..data + end].copy_from_slice(&row);
    put16(&mut after, data + DIRECTORY + count * 2, start as u16);
    put16(&mut after, data + 12, id as u16);
    put16(
        &mut after,
        data + 2,
        (start - DIRECTORY - (count + 1) * 2) as u16,
    );
    for offset in [16, 20, 67] {
        assert_eq!(
            u32::from_le_bytes(
                before[definition + offset..definition + offset + 4]
                    .try_into()
                    .unwrap()
            ) as usize,
            count
        );
        put32(&mut after, definition + offset, id);
    }

    let index = INDEX * PAGE;
    assert_eq!(word(before, index + 2), PAGE - ENTRIES - count * KEY_BYTES);
    let key = index + ENTRIES + count * KEY_BYTES;
    after[key] = 127;
    after[key + 1..key + 5].copy_from_slice(&(id ^ 0x8000_0000).to_be_bytes());
    after[key + 5..key + 9].copy_from_slice(&(((DATA as u32) << 8) | count as u32).to_be_bytes());
    let entry_end = (count + 1) * KEY_BYTES;
    after[index + BITMAP + entry_end / 8] |= 1 << (entry_end % 8);
    put16(&mut after, index + 2, (PAGE - ENTRIES - entry_end) as u16);
    after
}
