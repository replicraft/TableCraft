//! Literal source membership and complete fixed-patch expectations; no writer calls.
use super::{index_fixture, support};

pub const PAGE: usize = 4096;
pub const DATA: usize = 12;
pub const PATCH: usize = 13 * PAGE - 5;
pub const KEY_PATCH: usize = 13 * PAGE + 484;
pub const FOREIGN_ROOT: usize = 16;
pub const FOREIGN_MAP: usize = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    SameTable,
    Foreign,
    Orphan,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub bad: Vec<u8>,
    pub family: Family,
    pub claimed_map: usize,
    pub physical_start: Option<usize>,
}

fn foreign(bytes: &mut Vec<u8>, orphan: bool) -> usize {
    bytes.resize(17 * PAGE, 0);
    let definition = &mut bytes[7 * PAGE..8 * PAGE];
    assert_eq!(&definition[47..55], &[0; 8]);
    let names_end = 63
        + 8 * 25
        + [
            "szRelationship",
            "grbit",
            "ccolumn",
            "icolumn",
            "szReferencedObject",
            "szObject",
            "szReferencedColumn",
            "szColumn",
        ]
        .iter()
        .map(|name| 2 + name.len() * 2)
        .sum::<usize>();
    assert_eq!(names_end, 449);
    definition.copy_within(63..names_end, 75);
    definition[63..75].fill(0);
    support::put32(definition, 47, u32::from(!orphan));
    support::put32(definition, 51, 1);
    let physical = names_end + 12;
    definition[physical..].fill(0);
    for column in 0..10 {
        support::put16(definition, physical + 4 + column * 3, 0xffff);
    }
    support::put16(definition, physical + 4, 1);
    definition[physical + 6] = 1;
    support::put32(definition, physical + 34, (FOREIGN_MAP as u32) << 8);
    support::put32(definition, physical + 38, FOREIGN_ROOT as u32);
    definition[physical + 46] = 0x80;
    let mut tail = physical + 52;
    if !orphan {
        // One genuine ordinary logical owner, not a relationship or alias fiction.
        let name: Vec<_> = "RelationKey"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        tail += 28;
        support::put16(definition, tail, name.len() as u16);
        tail += 2;
        definition[tail..tail + name.len()].copy_from_slice(&name);
        tail += name.len();
    }
    support::put16(definition, tail, 0xffff);

    let mut members = vec![0; 69];
    members[5 + FOREIGN_ROOT / 8] |= 1 << (FOREIGN_ROOT % 8);
    let mut map = support::data_page(0, &[(0, members)]);
    support::put16(&mut map, 2, (PAGE - 14 - 2 - 69) as u16);
    bytes[FOREIGN_MAP * PAGE..(FOREIGN_MAP + 1) * PAGE].copy_from_slice(&map);
    let mut root = vec![0xa7; PAGE];
    root[..480].fill(0);
    root[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut root, 4, 7);
    support::put16(&mut root, 2, (PAGE - 480) as u16);
    bytes[FOREIGN_ROOT * PAGE..(FOREIGN_ROOT + 1) * PAGE].copy_from_slice(&root);
    physical
}

pub fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * PAGE + 14 + slot * 2;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    page * PAGE + usize::from(word)
}

pub fn case(code: u8, family: Family) -> Case {
    assert!([1, 5, 6].contains(&code));
    let mut before = index_fixture::database(false, true, false);
    if code != 1 {
        before[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    before[20] = code;
    let physical_start = if family == Family::SameTable {
        None
    } else {
        Some(foreign(&mut before, family == Family::Orphan))
    };
    let claimed_map = if family == Family::SameTable {
        14
    } else {
        FOREIGN_MAP
    };

    let mut expected = before.clone();
    assert_eq!(&before[PATCH..PATCH + 4], &10_i32.to_le_bytes());
    assert_eq!(before[KEY_PATCH], 10);
    expected[PATCH..PATCH + 4].copy_from_slice(&11_i32.to_le_bytes());
    expected[KEY_PATCH] = 11;
    let mut bad = before.clone();
    let map = row_start(&bad, claimed_map, 0);
    assert_eq!(bad[map + 5 + DATA / 8] & (1 << (DATA % 8)), 0);
    bad[map + 5 + DATA / 8] |= 1 << (DATA % 8);
    Case {
        before,
        expected,
        bad,
        family,
        claimed_map,
        physical_start,
    }
}
