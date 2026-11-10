//! Independent nullable field ADD at actual retained native definitions/catalog roots.
use crate::fixtures::access::{
    canonical_created_number::mixed::properties, canonical_table_drop::retained as dropped,
    packed_record::append_record, raw_forwarding_chain, raw_schema_add,
};

const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum Target {
    Number,
    ShortText,
}

impl Target {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Number => "NextNumber",
            Self::ShortText => "NextText",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Case {
    FirstNumber,
    SecondText,
    OriginalSparse,
}

pub const CASES: [Case; 3] = [Case::FirstNumber, Case::SecondText, Case::OriginalSparse];

pub struct Source {
    pub bytes: Vec<u8>,
    pub definition: u32,
    pub catalog_root: usize,
    pub roots: Vec<usize>,
}

pub fn source(case: Case) -> Source {
    let original = dropped::source(match case {
        Case::FirstNumber => dropped::Case::FirstNumber,
        Case::SecondText => dropped::Case::SecondText,
        Case::OriginalSparse => dropped::Case::OriginalSparse,
    });
    Source {
        bytes: original.bytes,
        definition: original.definition,
        catalog_root: original.catalog_root,
        roots: original.roots,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Old records remain byte-identical and the new logical native field is NULL.
/// A new complete MR2 snapshot is appended; old inline/body bytes remain history.
pub fn after(before: &[u8], definition: u32, catalog_root: usize, target: Target) -> Vec<u8> {
    let range = raw_forwarding_chain::terminal_range(before, 4, catalog_root);
    let row = &before[range.clone()];
    assert_eq!(&row[..2], &[17, 0]);
    assert_eq!(
        u32::from_le_bytes(row[2..6].try_into().unwrap()),
        definition
    );
    let footer = row.len() - 5;
    let start = word(row, footer - 18);
    let end = word(row, footer - 20);
    assert!(end - start >= 12);
    let tagged = u32::from_le_bytes(row[start..start + 4].try_into().unwrap());
    let length = (tagged & 0x3fff_ffff) as usize;
    let body = if tagged & 0xc000_0000 == 0x8000_0000 {
        assert_eq!(end - start, 12 + length);
        row[start + 12..end].to_vec()
    } else {
        assert_eq!(tagged & 0xc000_0000, 0x4000_0000);
        assert_eq!(end - start, 12);
        let reference = u32::from_le_bytes(row[start + 4..start + 8].try_into().unwrap());
        assert_eq!(reference >> 8, 10);
        let slot = (reference & 255) as usize;
        let begin = word(before, 10 * PAGE + 14 + 2 * slot);
        let finish = if slot == 0 {
            PAGE
        } else {
            word(before, 10 * PAGE + 12 + 2 * slot)
        };
        assert_eq!(finish - begin, length);
        before[10 * PAGE + begin..10 * PAGE + finish].to_vec()
    };
    let (kind, body) = match target {
        Target::Number => (
            raw_schema_add::Kind::Number,
            properties::append_number(&body, target.name()),
        ),
        Target::ShortText => (
            raw_schema_add::Kind::ShortText,
            properties::append_text(&body, target.name()),
        ),
    };
    let mut after = raw_schema_add::append(before, definition, target.name(), kind);
    let slot = word(before, 10 * PAGE + 12);
    append_record(&mut after[10 * PAGE..11 * PAGE], &body);
    let descriptor = range.start + start;
    after[descriptor..descriptor + 4]
        .copy_from_slice(&(0x4000_0000 | body.len() as u32).to_le_bytes());
    after[descriptor + 4..descriptor + 8]
        .copy_from_slice(&((10u32 << 8) | slot as u32).to_le_bytes());
    after[descriptor + 8..descriptor + 12].fill(0);
    // The twelve-byte descriptor consumes less than the prior inline footprint.
    // Preserve ignored original inline bytes and the complete physical row size.
    for variable in 9..=11 {
        let at = range.start + footer - 2 * (variable + 1);
        after[at..at + 2].copy_from_slice(&((start + 12) as u16).to_le_bytes());
    }
    after[range.end - 2] |= 0x40;
    after
}
