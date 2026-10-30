//! Pure additive third-variable literal constructor; no producer authority.
use super::model::{self, Case, Family, Recipe};

pub fn case(code: u8, family: Family, descending: bool) -> Case {
    let mut case = model::fitted(code, family, descending, Recipe::Permutation);
    for (ordinal, replacement) in [
        (0, vec![0x11, 0x22, 0x33, 0x44]),
        (3, vec![0x91, 0x82, 0x73, 0x64, 0x55]),
    ] {
        let record = &mut case.final_records[ordinal];
        let footer = record.len() - 11;
        let old_end = usize::from(u16::from_le_bytes(
            record[footer..footer + 2].try_into().unwrap(),
        ));
        let start = usize::from(u16::from_le_bytes(
            record[footer + 2..footer + 4].try_into().unwrap(),
        ));
        assert_eq!(old_end - start, 3);
        assert!(start + replacement.len() <= footer);

        // The last variable slot has no following payload to relocate. Padding
        // stays at its original absolute addresses outside the new active span.
        record[start..start + replacement.len()].copy_from_slice(&replacement);
        let new_end = u16::try_from(start + replacement.len()).unwrap();
        record[footer..footer + 2].copy_from_slice(&new_end.to_le_bytes());
        let mask = record.last_mut().unwrap();
        assert_eq!(*mask & 0xa0, 0xa0);
        *mask |= 16;
        case.final_rows[ordinal].opaque = Some(replacement);

        let page = if code == 6 && ordinal == 0 { 17 } else { 12 };
        let slot = if page == 17 { 0 } else { ordinal };
        let directory = page * 4096 + 14 + slot * 2;
        let offset = usize::from(
            u16::from_le_bytes(case.expected[directory..directory + 2].try_into().unwrap())
                & 0x1fff,
        );
        case.expected[page * 4096 + offset..page * 4096 + offset + record.len()]
            .copy_from_slice(record);
    }
    case.changed = vec![0, 1, 3];
    case
}
