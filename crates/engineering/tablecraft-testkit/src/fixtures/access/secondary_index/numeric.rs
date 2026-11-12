//! Independent numeric keys and complete retained-index images, without engine codecs.
use super::normal::{self, definition, tree};
use crate::fixtures::access::{
    canonical_retained_add_table::{extend_global_map, fields, populated},
    index_tuples::numeric_fixture,
    packed_record::append_record,
    put16, put32,
};
use std::collections::BTreeSet;

const PAGE: usize = 4096;

/// Native scalar categories with separate literal storage and key encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    Currency,
    Single,
    Double,
    DateTime,
}

pub const KINDS: [Kind; 9] = [
    Kind::Boolean,
    Kind::Byte,
    Kind::Int16,
    Kind::Int32,
    Kind::Int64,
    Kind::Currency,
    Kind::Single,
    Kind::Double,
    Kind::DateTime,
];

impl Kind {
    /// Literal Jet/ACE native type; no production ScalarType mapping is used.
    pub const fn native(self) -> u8 {
        match self {
            Self::Boolean => 1,
            Self::Byte => 2,
            Self::Int16 => 3,
            Self::Int32 => 4,
            Self::Int64 => 19,
            Self::Currency => 5,
            Self::Single => 6,
            Self::Double => 7,
            Self::DateTime => 8,
        }
    }
}

/// Large physical topologies use Int64, whose entire ordered width is exercised.
#[derive(Clone, Copy, Debug)]
pub enum Population {
    Empty,
    NullableDuplicate,
    ThreeLeaves,
    MultiLevel,
}

/// Original carrier bits and literal physical ROW identity, in DATA source order.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub value: Option<i64>,
    pub reference: u32,
}

/// Original and complete independently authored numeric-index image.
pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub kind: Kind,
    pub table: &'static str,
    pub column: &'static str,
    pub index_name: &'static str,
    pub target_definition: u32,
    pub native_number: u16,
    pub root: u32,
    pub index_pages: Vec<u32>,
    pub leaf_pages: usize,
    pub levels: usize,
    pub map_host: u32,
    pub rows: Vec<Row>,
    pub descending: bool,
}

fn values(kind: Kind, population: Population) -> Vec<Option<i64>> {
    match population {
        Population::Empty => Vec::new(),
        Population::NullableDuplicate => match kind {
            Kind::Boolean => vec![Some(0), Some(-1), Some(-1), Some(0)],
            Kind::Byte => vec![None, Some(0), Some(255), Some(17), Some(17), None],
            Kind::Int16 => vec![
                None,
                Some(-32_768),
                Some(32_767),
                Some(0),
                Some(-7),
                Some(-7),
            ],
            Kind::Int32 => vec![
                None,
                Some(-2_147_483_648),
                Some(2_147_483_647),
                Some(0),
                Some(-7),
                Some(-7),
            ],
            Kind::Int64 => vec![
                None,
                Some(i64::MIN),
                Some(i64::MAX),
                Some(0),
                Some(-9_007_199_254_740_993),
                Some(-9_007_199_254_740_993),
            ],
            Kind::Currency => vec![
                None,
                Some(i64::MIN),
                Some(i64::MAX),
                Some(0),
                Some(-12_345),
                Some(-12_345),
                Some(12_345),
            ],
            Kind::Single => vec![
                None,
                Some(0x8000_0000),
                Some(0),
                Some(1),
                Some(0x8000_0001),
                Some(0xbf80_0000),
                Some(0x3f80_0000),
                Some(0x3f80_0000),
                Some(0x7f7f_ffff),
                Some(0xff7f_ffff),
            ],
            Kind::Double => vec![
                None,
                Some(i64::MIN),
                Some(0),
                Some(1),
                Some(0x8000_0000_0000_0001_u64 as i64),
                Some(0xbff0_0000_0000_0000_u64 as i64),
                Some(0x3ff0_0000_0000_0000),
                Some(0x3ff0_0000_0000_0000),
                Some(0x7fef_ffff_ffff_ffff),
                Some(0xffef_ffff_ffff_ffff_u64 as i64),
            ],
            Kind::DateTime => vec![
                None,
                Some(i64::MIN),
                Some(0),
                Some(0xbff4_0000_0000_0000_u64 as i64),
                Some(0xc004_0000_0000_0000_u64 as i64),
                Some(0x4004_0000_0000_0000),
                Some(0x4004_0000_0000_0000),
            ],
        },
        Population::ThreeLeaves | Population::MultiLevel => {
            assert_eq!(kind, Kind::Int64, "explicit eight-byte topology population");
            let count: usize = if matches!(population, Population::ThreeLeaves) {
                3_000
            } else {
                200_000
            };
            (0..count)
                .map(|ordinal| {
                    if ordinal.is_multiple_of(17) {
                        None
                    } else {
                        let rank = (ordinal / 2) as u128;
                        let bits = (rank * u128::from(u64::MAX) / (count / 2 - 1) as u128) as u64;
                        Some((bits ^ 0x8000_0000_0000_0000) as i64)
                    }
                })
                .collect()
        }
    }
}

/// One ascending/descending component; the ROW suffix is never complemented.
/// IEEE values are supplied as exact finite bits and preserve signed zero.
pub fn key(kind: Kind, value: Option<i64>, descending: bool) -> Vec<u8> {
    if kind == Kind::Boolean {
        assert!(matches!(value, Some(-1 | 0)), "Boolean has no nullable key");
    }

    let mut key = vec![if value.is_some() { 127 } else { 0 }];
    if let Some(value) = value {
        let bytes = match kind {
            Kind::Boolean => vec![if value == -1 { 0 } else { 255 }],
            Kind::Byte => vec![u8::try_from(value).unwrap()],
            Kind::Int16 => ((i16::try_from(value).unwrap() as u16) ^ 0x8000)
                .to_be_bytes()
                .to_vec(),
            Kind::Int32 => ((i32::try_from(value).unwrap() as u32) ^ 0x8000_0000)
                .to_be_bytes()
                .to_vec(),
            Kind::Int64 | Kind::Currency => ((value as u64) ^ 0x8000_0000_0000_0000)
                .to_be_bytes()
                .to_vec(),
            Kind::Single => {
                let bits = u32::try_from(value).expect("literal finite Single bits");
                assert!(f32::from_bits(bits).is_finite());
                numeric_fixture::floating_key(6, value)
            }
            Kind::Double | Kind::DateTime => {
                assert!(f64::from_bits(value as u64).is_finite());
                numeric_fixture::floating_key(kind.native(), value)
            }
        };
        key.extend(bytes);
    }

    if descending {
        for byte in &mut key {
            *byte = !*byte;
        }
    }

    key
}

/// Same-table LONG index and Currency/Unicode companions are protected in every case.
/// Small populations cover all nine categories; large populations require Int64.
pub fn source(kind: Kind, population: Population, descending: bool) -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let owner = (seed.len() / PAGE) as u32;
    let values = values(kind, population);
    let literal = populated::numeric::source(&seed, "NumericValues", kind.native(), &values);
    let guard_rows = literal
        .references
        .iter()
        .enumerate()
        .map(|(ordinal, reference)| normal::Row {
            score: Some((ordinal % 31) as i32 - 15),
            reference: *reference,
        })
        .collect();
    let guard = normal::finish(
        literal.bytes,
        owner,
        "NumericValues",
        "Guard",
        1,
        guard_rows,
        false,
    );
    let rows = values
        .into_iter()
        .zip(literal.references)
        .map(|(value, reference)| Row { value, reference })
        .collect();
    let result = finish(guard.expected, owner, kind, rows, descending);
    if matches!(population, Population::ThreeLeaves) {
        assert!(result.leaf_pages > 2);
    }
    if matches!(population, Population::MultiLevel) {
        assert!(result.levels >= 3);
    }
    result
}

fn finish(before: Vec<u8>, owner: u32, kind: Kind, rows: Vec<Row>, descending: bool) -> Source {
    let entries = rows
        .iter()
        .map(|row| (key(kind, row.value, descending), row.reference))
        .collect::<Vec<_>>();
    let first = (before.len() / PAGE) as u32;
    let tree = tree::build_encoded(&entries, first, owner);
    let map_host = first + tree.pages.len() as u32;
    let index_pages = (first..map_host).collect::<Vec<_>>();
    let map_bytes = 5 + 64_usize.max(index_pages.len().div_ceil(8));
    assert!(map_bytes <= PAGE - 16);
    let mut map = vec![0; map_bytes];
    put32(&mut map, 1, first);
    for offset in 0..index_pages.len() {
        map[5 + offset / 8] |= 1 << (offset % 8);
    }
    let mut container = vec![0; PAGE];
    container[..2].copy_from_slice(&[1, 1]);
    put16(&mut container, 2, (PAGE - 14) as u16);
    append_record(&mut container, &map);

    let distinct = entries
        .iter()
        .map(|(key, _)| key.as_slice())
        .collect::<BTreeSet<_>>()
        .len();
    let declaration = definition::append(
        &before[owner as usize * PAGE..(owner as usize + 1) * PAGE],
        definition::NewIndex {
            name: "NumericLookup",
            column: 0,
            descending,
            usage: map_host << 8,
            root: tree.root,
            distinct: distinct as u32,
        },
    );
    // NO_CLONE: An independent complete expected image coexists with SOURCE.
    // The raw oracle must retain both immutable byte sequences for equality checks.
    let mut expected = before.to_vec();
    expected[owner as usize * PAGE..(owner as usize + 1) * PAGE].copy_from_slice(&declaration);
    for page in tree.pages {
        expected.extend(page);
    }
    expected.extend(container);
    extend_global_map(&mut expected, before.len() / PAGE);
    for page in 0..before.len() / PAGE {
        if page != 1 && page != owner as usize {
            assert_eq!(
                &expected[page * PAGE..(page + 1) * PAGE],
                &before[page * PAGE..(page + 1) * PAGE]
            );
        }
    }

    Source {
        before,
        expected,
        kind,
        table: "NumericValues",
        column: "KeyValue",
        index_name: "NumericLookup",
        target_definition: owner,
        native_number: 0,
        root: tree.root,
        index_pages,
        leaf_pages: tree.leaf_pages,
        levels: tree.levels,
        map_host,
        rows,
        descending,
    }
}

#[cfg(test)]
mod tests;
