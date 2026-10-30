//! Literal physical owners, aliases, column metadata and declaration grammar.
use super::{Family, ROWS, base};
use tablecraft_testkit::access as raw;

pub(super) fn definition(family: Family, descending: bool) -> Vec<u8> {
    let mut page = base::definition(
        &[
            ("Id", 4, 0, 4),
            ("Age", 4, 4, 4),
            (
                "Name",
                if matches!(family, Family::Binary) {
                    9
                } else {
                    10
                },
                0,
                if matches!(family, Family::Binary) {
                    255
                } else {
                    510
                },
            ),
        ],
        ROWS as u32,
        11 << 8,
    );
    page.copy_within(63..162, 87);
    page[63..87].fill(0);
    raw::put32(&mut page, 67, 701);
    raw::put32(&mut page, 79, 901);
    raw::put16(&mut page, 43, 1);
    raw::put32(&mut page, 47, 4);
    raw::put32(&mut page, 51, 2);
    raw::put32(&mut page, 59, (11 << 8) | 1);
    let name = 87 + 50;
    page[name + 15] = 2;
    raw::put16(&mut page, name + 7, 0);
    if !matches!(family, Family::Binary) {
        page[name + 11..name + 15].copy_from_slice(&[
            9,
            4,
            0,
            u8::from(matches!(family, Family::Modern)),
        ]);
    }
    for (physical, root) in [13, 15].into_iter().enumerate() {
        let at = 186 + physical * 52;
        page[at..at + 52].fill(0);
        for component in 0..10 {
            raw::put16(&mut page, at + 4 + component * 3, 0xffff);
        }
        raw::put16(&mut page, at + 4, 2);
        page[at + 6] = u8::from(!descending);
        if physical == 1 {
            raw::put16(&mut page, at + 7, 1);
            page[at + 9] = u8::from(descending);
        }
        raw::put32(&mut page, at + 34, (14 << 8) | physical as u32);
        raw::put32(&mut page, at + 38, root);
        page[at + 46] = if physical == 1 { 0x81 } else { 0x80 };
    }
    let mut names = 402;
    for (ordinal, name) in ["NameKey", "NameAlias", "PairKey", "PairAlias"]
        .iter()
        .enumerate()
    {
        raw::put32(&mut page, 290 + ordinal * 28 + 4, ordinal as u32);
        raw::put32(&mut page, 290 + ordinal * 28 + 8, (ordinal / 2) as u32);
        let bytes = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut page, names, bytes.len() as u16);
        names += 2;
        page[names..names + bytes.len()].copy_from_slice(&bytes);
        names += bytes.len();
    }
    assert_eq!(names, 474);
    raw::put16(&mut page, names, 0xffff);
    page
}
