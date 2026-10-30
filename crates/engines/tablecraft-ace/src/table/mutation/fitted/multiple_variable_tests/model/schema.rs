//! Source-bound native field numbers, two physical tuples and four aliases.
use super::{Family, base};
use tablecraft_testkit::access as raw;

pub(super) fn definition(rows: usize, family: Family, descending: bool) -> Vec<u8> {
    let fields = [
        ("Id", 4, 0, 4),
        ("Age", 4, 4, 4),
        (
            "First",
            if family.first_text() { 10 } else { 9 },
            0,
            if family.first_text() { 510 } else { 255 },
        ),
        (
            "Second",
            if family.second_text() { 10 } else { 9 },
            0,
            if family.second_text() { 510 } else { 255 },
        ),
        ("Opaque", 9, 0, 32),
    ];
    let mut page = base::definition(&fields, rows as u32, 11 << 8);
    let names_end = 63
        + 125
        + fields
            .iter()
            .map(|(name, ..)| 2 + name.len() * 2)
            .sum::<usize>();
    page.copy_within(63..names_end, 87);
    page[63..87].fill(0);
    raw::put32(&mut page, 67, 701);
    raw::put32(&mut page, 79, 901);
    raw::put16(&mut page, 43, 3);
    raw::put32(&mut page, 47, 4);
    raw::put32(&mut page, 51, 2);
    raw::put32(&mut page, 59, (11 << 8) | 1);

    for position in 2..5 {
        let at = 87 + position * 25;
        page[at + 15] = 2;
        raw::put16(&mut page, at + 7, (position - 2) as u16);
        if position == 2 && family.first_text() || position == 3 && family.second_text() {
            page[at + 11..at + 15].copy_from_slice(&[
                9,
                4,
                0,
                u8::from(matches!(family, Family::ModernBinary)),
            ]);
            page[at + 16] = 1;
        }
    }
    let physical = names_end + 24;
    for (number, root) in [13, 15].into_iter().enumerate() {
        let at = physical + number * 52;
        page[at..at + 52].fill(0);
        for component in 0..10 {
            raw::put16(&mut page, at + 4 + component * 3, 0xffff);
        }
        let columns = if number == 0 {
            vec![(2, descending), (3, !descending)]
        } else {
            vec![(3, !descending), (2, descending), (1, !descending)]
        };
        for (component, (field, descending)) in columns.into_iter().enumerate() {
            raw::put16(&mut page, at + 4 + component * 3, field);
            page[at + 6 + component * 3] = u8::from(!descending);
        }
        raw::put32(&mut page, at + 34, (14 << 8) | number as u32);
        raw::put32(&mut page, at + 38, root);
        page[at + 46] = if number == 0 { 0x81 } else { 0x80 };
    }
    let logical = physical + 104;
    let mut names = logical + 112;
    for (ordinal, name) in ["Pair", "PairAlias", "Reverse", "ReverseAlias"]
        .iter()
        .enumerate()
    {
        raw::put32(&mut page, logical + ordinal * 28 + 4, ordinal as u32);
        raw::put32(&mut page, logical + ordinal * 28 + 8, (ordinal / 2) as u32);
        let bytes = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut page, names, bytes.len() as u16);
        names += 2;
        page[names..names + bytes.len()].copy_from_slice(&bytes);
        names += bytes.len();
    }
    raw::put16(&mut page, names, 0xffff);
    page
}
