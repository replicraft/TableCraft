//! Explicit LONG AutoNumber declarations, with mode deliberately unclassified.
use super::{
    canonical_creation as plain, canonical_creation_primary as primary,
    canonical_native17_creation as native17, data_page, put16, put32,
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub enum Case {
    ManualHighLast,
    EmptyUnindexed,
    EmptyPrimary,
    HighLast,
    DeletedHigh,
    NegativeEmpty,
    MaximumEmpty,
}

pub const CASES: [Case; 7] = [
    Case::ManualHighLast,
    Case::EmptyUnindexed,
    Case::EmptyPrimary,
    Case::HighLast,
    Case::DeletedHigh,
    Case::NegativeEmpty,
    Case::MaximumEmpty,
];

impl Case {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ManualHighLast => "manual-high-last",
            Self::EmptyUnindexed => "unknown-empty-unindexed",
            Self::EmptyPrimary => "unknown-empty-primary",
            Self::HighLast => "unknown-high-last",
            Self::DeletedHigh => "unknown-deleted-high",
            Self::NegativeEmpty => "unknown-negative-empty",
            Self::MaximumEmpty => "unknown-max-empty",
        }
    }
}

pub struct Source {
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub physical: Vec<(i32, i32, bool)>,
    pub last: i32,
    pub autonumber: bool,
    pub primary: bool,
    pub history: u32,
}

fn record(id: i32, score: i32) -> Vec<u8> {
    let mut raw = 2_u16.to_le_bytes().to_vec();
    raw.extend(id.to_le_bytes());
    raw.extend(score.to_le_bytes());
    raw.push(3);
    assert_eq!(raw.len(), 11);
    raw
}

fn leaf(physical: &[(i32, i32, bool)]) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[4, 1]);
    put32(&mut page, 4, 7);
    let mut entries = physical
        .iter()
        .enumerate()
        .filter(|(_, (_, _, deleted))| !deleted)
        .map(|(slot, (id, _, _))| (*id, (8 << 8) | slot as u32))
        .collect::<Vec<_>>();
    entries.sort_unstable();
    let mut active = 0;
    for (id, reference) in entries {
        let mut wire = vec![127];
        wire.extend(((id as u32) ^ 0x8000_0000).to_be_bytes());
        wire.extend(reference.to_be_bytes());
        page[480 + active..480 + active + 9].copy_from_slice(&wire);
        active += 9;
        page[27 + active / 8] |= 1 << (active % 8);
    }
    put16(&mut page, 2, (3616 - active) as u16);
    page
}

/// The source is authored from independent Empty envelopes, never a creator,
/// mode detector, incrementing preview, native mutation or SDK writer output.
pub fn source(case: Case) -> Source {
    let indexed = !matches!(case, Case::ManualHighLast | Case::EmptyUnindexed);
    let autonumber = !matches!(case, Case::ManualHighLast);
    let last = match case {
        Case::ManualHighLast | Case::HighLast => 137,
        Case::DeletedHigh => 3,
        Case::NegativeEmpty => -2,
        Case::MaximumEmpty => i32::MAX,
        _ => 0,
    };
    let physical = match case {
        Case::ManualHighLast | Case::HighLast => vec![(1, 10, false)],
        Case::DeletedHigh => vec![(1, 10, false), (2, 20, false), (3, 30, true)],
        _ => Vec::new(),
    };
    let rows = physical
        .iter()
        .filter(|(_, _, deleted)| !deleted)
        .map(|(id, score, _)| vec![Value::Int32(*id), Value::Int32(*score)])
        .collect::<Vec<_>>();
    let mut bytes = if indexed {
        primary::source(primary::Case::Empty).bytes
    } else {
        native17::source(plain::source(plain::Case::Empty).bytes)
    };
    let field_start = if indexed { 75 } else { 63 };
    assert_eq!(bytes[7 * 4096 + field_start], 4);
    assert_eq!(bytes[7 * 4096 + field_start + 15], 3);
    if autonumber {
        bytes[7 * 4096 + field_start + 15] = 7;
    }
    bytes[7 * 4096 + 20..7 * 4096 + 24].copy_from_slice(&last.to_le_bytes());
    put32(&mut bytes, 7 * 4096 + 16, rows.len() as u32);
    let records = physical
        .iter()
        .map(|(id, score, deleted)| (if *deleted { 0xc000 } else { 0 }, record(*id, *score)))
        .collect::<Vec<_>>();
    let mut data = data_page(7, &records);
    put16(&mut data, 2, (4096 - 14 - 13 * records.len()) as u16);
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data);
    let history = if indexed { physical.len() as u32 } else { 0 };
    if indexed {
        put32(&mut bytes, 7 * 4096 + 67, history);
        bytes[9 * 4096..10 * 4096].copy_from_slice(&leaf(&physical));
    }
    Source {
        bytes,
        rows,
        physical,
        last,
        autonumber,
        primary: indexed,
        history,
    }
}
