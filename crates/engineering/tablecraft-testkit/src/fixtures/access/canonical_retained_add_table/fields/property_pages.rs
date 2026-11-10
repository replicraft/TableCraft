//! Actual allocated/hinted LVAL alternatives with complete retained body records.
use super::{Case as SourceCase, PAGE, Target, after_on_property_page, word};
use crate::fixtures::access::{database_properties, packed_record::append_record, put16};

#[derive(Clone, Copy, Debug)]
pub enum Case {
    HighestInsufficient,
    OriginalInsufficient,
}

pub const CASES: [Case; 2] = [Case::HighestInsufficient, Case::OriginalInsufficient];

pub struct Source {
    pub bytes: Vec<u8>,
    pub definition: u32,
    pub catalog_root: usize,
    pub chosen_page: u32,
    pub other_page: u32,
    pub owned_cell: usize,
    pub hint_cell: usize,
    pub mask: u8,
}

pub fn source(case: Case) -> Source {
    let original = super::source(SourceCase::FirstNumber);
    let mut bytes = original.bytes;
    let new_page = bytes.len() / PAGE;
    assert!(new_page < 64);
    bytes.resize(bytes.len() + PAGE, 0);
    let page = &mut bytes[new_page * PAGE..(new_page + 1) * PAGE];
    page[..2].copy_from_slice(&[1, 1]);
    put16(page, 2, (PAGE - 14) as u16);
    page[4..8].copy_from_slice(b"LVAL");
    let retired = database_properties::name_auto_correct_disabled();
    append_record(page, &retired);
    let mut cells = [0; 2];
    let mask = 1 << (new_page % 8);
    for (index, slot) in [16, 17].into_iter().enumerate() {
        let start = word(&bytes, 3 * PAGE + 14 + 2 * slot);
        assert_eq!(&bytes[3 * PAGE + start..3 * PAGE + start + 5], &[0; 5]);
        let cell = 3 * PAGE + start + 5 + new_page / 8;
        assert_eq!(bytes[cell] & mask, 0);
        bytes[cell] |= mask;
        cells[index] = cell;
    }
    let global = word(&bytes, PAGE + 14);
    bytes[PAGE + global + 5 + new_page / 8] &= !mask;
    let (full, chosen) = match case {
        Case::HighestInsufficient => (new_page, 10),
        Case::OriginalInsufficient => (10, new_page),
    };
    let page = &mut bytes[full * PAGE..(full + 1) * PAGE];
    while word(page, 2) >= retired.len() + 2 {
        append_record(page, &retired);
    }
    Source {
        bytes,
        definition: original.definition,
        catalog_root: original.catalog_root,
        chosen_page: chosen as u32,
        other_page: full as u32,
        owned_cell: cells[0],
        hint_cell: cells[1],
        mask,
    }
}

pub fn after(source: &Source, target: Target) -> Vec<u8> {
    let full = source.other_page as usize * PAGE;
    let after = after_on_property_page(
        &source.bytes,
        source.definition,
        source.catalog_root,
        target,
        source.chosen_page,
    );
    let row = crate::fixtures::access::raw_forwarding_chain::record(&after, 4, source.catalog_root);
    let descriptor = word(row, row.len() - 5 - 18);
    let body_bytes = (u32::from_le_bytes(row[descriptor..descriptor + 4].try_into().unwrap())
        & 0x3fff_ffff) as usize;
    assert!(body_bytes + 2 > word(&source.bytes, full + 2));
    after
}
