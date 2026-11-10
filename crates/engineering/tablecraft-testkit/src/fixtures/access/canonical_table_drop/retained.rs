//! Independent logical table deletion beside retained live and retired storage.
use super::current as dropped;
use crate::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_delete,
        deleted_field::populated::{Case as FieldCase, readdition::Target as Field},
    },
    canonical_retained_add_table::{
        self as added, Target, deletes,
        rows::{self, Input, Profile},
    },
    raw_forwarding_chain, raw_record_delete,
};

const PAGE: usize = 4096;
const CATALOG: usize = 4 * PAGE;
const CATALOG_LIVE: usize = 2 * PAGE + 16;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    FirstNumber,
    SecondText,
    OriginalSparse,
    EmptyNumber,
}

pub const CASES: [Case; 4] = [
    Case::FirstNumber,
    Case::SecondText,
    Case::OriginalSparse,
    Case::EmptyNumber,
];

pub struct Source {
    pub bytes: Vec<u8>,
    pub definition: u32,
    pub catalog_root: usize,
    pub roots: Vec<usize>,
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Sources retain independently authored empty permission/relation declarations.
/// The original sparse case also proves exact commutation with sibling append.
pub fn source(case: Case) -> Source {
    if matches!(case, Case::OriginalSparse) {
        let original =
            current_delete::source(FieldCase::MixedBinary, Field::ShortText, true, true, true);
        let bytes = added::desktop::append(&original.bytes, "Other", Target::GeneratedNumber);
        let removed = current_delete::after(&original.bytes, &original.roots);
        let expected = catalog_tombstone(
            &added::desktop::append(&removed, "Other", Target::GeneratedNumber),
            7,
            7,
        );
        assert_eq!(after(&bytes, 7, 7, &original.roots), expected);
        return Source {
            bytes,
            definition: 7,
            catalog_root: 7,
            roots: original.roots,
        };
    }
    let original = dropped::source(
        FieldCase::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let mut bytes = dropped::after(&original);
    let second = matches!(case, Case::SecondText);
    if second {
        bytes = added::desktop::append(&bytes, "Other", Target::GeneratedNumber);
        bytes = rows::append(&bytes, 11, Profile::DesktopNumber, Input::Number(99));
    }
    let definition = (bytes.len() / PAGE) as u32;
    let catalog_root = word(&bytes, CATALOG + 12);
    let (target, profile) = if second {
        (Target::GeneratedText, Profile::DesktopText)
    } else {
        (Target::GeneratedNumber, Profile::DesktopNumber)
    };
    bytes = added::desktop::append(&bytes, "Table1", target);
    let roots = if matches!(case, Case::EmptyNumber) {
        Vec::new()
    } else {
        for input in if second {
            [Input::Text("日本語🦀"), Input::Null]
        } else {
            [Input::Number(17), Input::Null]
        } {
            bytes = rows::append(&bytes, definition, profile, input);
        }
        vec![0, 1]
    };
    if !second {
        bytes = added::desktop::append(&bytes, "Other", Target::GeneratedText);
    } else {
        bytes = raw_forwarding_chain::extend(&bytes, definition + 1, 0, 20);
        bytes = raw_forwarding_chain::extend(&bytes, 4, catalog_root, 20);
    }
    Source {
        bytes,
        definition,
        catalog_root,
        roots,
    }
}

/// Every actual live row is removed before its catalog logical root is hidden.
/// Allocated pages, physical slots, LAST, inactive keys and MR2 bodies survive.
pub fn after(before: &[u8], definition: u32, catalog_root: usize, roots: &[usize]) -> Vec<u8> {
    let deleted = if definition == 7 {
        // Reuse the independent raw operation owner, without widening the public
        // original eleven-page fixture contract. source() verifies commutation.
        raw_record_delete::after(before, definition, roots)
    } else {
        deletes::after(before, definition, roots)
    };
    let live = definition as usize * PAGE + 16;
    assert_eq!(&deleted[live..live + 4], &[0; 4]);
    catalog_tombstone(&deleted, definition, catalog_root)
}

fn catalog_tombstone(before: &[u8], definition: u32, catalog_root: usize) -> Vec<u8> {
    let mut matching = Vec::new();
    for root in 0..word(before, CATALOG + 12) {
        let entry = word(before, CATALOG + 14 + 2 * root);
        if entry & 0x8000 != 0 {
            continue;
        }
        let record = raw_forwarding_chain::record(before, 4, root);
        assert_eq!(&record[..2], &[17, 0]);
        let id = u32::from_le_bytes(record[2..6].try_into().unwrap());
        if id == definition {
            matching.push(root);
        }
    }
    assert_eq!(matching, [catalog_root]);
    let directory = CATALOG + 14 + 2 * catalog_root;
    let old = word(before, directory) as u16;
    assert_eq!(old & 0x8000, 0);
    let count = u32::from_le_bytes(before[CATALOG_LIVE..CATALOG_LIVE + 4].try_into().unwrap());
    let mut after = before.to_vec();
    after[directory..directory + 2].copy_from_slice(&(old | 0xc000).to_le_bytes());
    after[CATALOG_LIVE..CATALOG_LIVE + 4]
        .copy_from_slice(&count.checked_sub(1).unwrap().to_le_bytes());
    after
}
