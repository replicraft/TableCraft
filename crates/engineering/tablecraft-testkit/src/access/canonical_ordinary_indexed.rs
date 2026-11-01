//! Independent ordinary counter/index SOURCE images, never write authority.
//!
//! The only reused images are canonical original creation/variable envelopes.
//! Counter declarations, index definitions, keys, row suffixes and page roles
//! are authored below. No event is removed, and no production encoder is used.
use super::{canonical_creation, canonical_variable_creation, data_page, put16, put32};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DEFINITION: u32 = 7;
const DATA: u32 = 8;
const ENTRY_START: usize = 480;
const ENTRY_CAPACITY: usize = 3616;

/// Finite originals and adversarial representation controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Readonly generated Id, LAST137, no index, editable Score.
    CounterOnly,
    /// Primary Id with LAST−7 distinct from live IDs1/2.
    CounterPrimary,
    /// Generated primary Id beside unchanged Text/Binary/Decimal and editable Flag.
    VariablePrimary,
    /// Ordinary include-NULL Id index, inactive Int32 payload2 retained.
    NullableOrdinary,
    /// Ordinary UNIQUE manual Id index, editable unindexed Score.
    UniqueManual,
    /// Two ordinary logical names share one physical tree with history99.
    AliasHistory,
    /// Primary leaf95, with a nonzero index-map base and explicit free holes.
    RelocatedPrimary,
    /// Root9 explicitly bounds left10 and has right-tail11; prefixes are4.
    RetainedTree,
    /// Generated flags on a native Int16 declaration.
    WrongCounterType,
    /// Both Id and Score claim generated Int32 flags.
    SecondCounter,
    /// An unknown additional generator flag is present.
    UnknownCounterFlag,
    /// One physical index has no logical declaration.
    OrphanPhysical,
    /// A second alias uses a foreign logical kind.
    IncompatibleAlias,
    /// Ignore-NULL physical policy is outside this grammar.
    IgnoreNulls,
    /// Index and DATA ownership select one physical map record.
    SharedMapCell,
    /// Index-owned membership includes a live DATA page.
    IndexDataOverlap,
    /// GLOBAL declares the reachable leaf free.
    FreeIndex,
    /// Reachable leaf9 is absent from its physical owned map.
    MissingOwned,
    /// Owned page10 is unreachable from the leaf9 root.
    UnreachableOwned,
    /// A stored key differs from its DATA cell.
    WrongKey,
    /// A correct key carries the wrong original logical row suffix.
    WrongRow,
    /// An explicit node bound falls below its left child's maximum.
    BadNodeBound,
    /// A reserved index-header byte is unexplained.
    UnknownIndexHeader,
    /// A declared inactive leaf extent retains a nonzero A7 fill byte-for-byte.
    InactiveIndexTail,
}

/// Nine positive hypotheses and fifteen refusal controls in authoring order.
pub const CASES: [Case; 24] = [
    Case::CounterOnly,
    Case::CounterPrimary,
    Case::VariablePrimary,
    Case::NullableOrdinary,
    Case::UniqueManual,
    Case::AliasHistory,
    Case::RelocatedPrimary,
    Case::RetainedTree,
    Case::WrongCounterType,
    Case::SecondCounter,
    Case::UnknownCounterFlag,
    Case::OrphanPhysical,
    Case::IncompatibleAlias,
    Case::IgnoreNulls,
    Case::SharedMapCell,
    Case::IndexDataOverlap,
    Case::FreeIndex,
    Case::MissingOwned,
    Case::UnreachableOwned,
    Case::WrongKey,
    Case::WrongRow,
    Case::BadNodeBound,
    Case::UnknownIndexHeader,
    Case::InactiveIndexTail,
];

impl Case {
    /// Stable export identity.
    pub const fn id(self) -> &'static str {
        match self {
            Self::CounterOnly => "counter-only",
            Self::CounterPrimary => "counter-primary",
            Self::VariablePrimary => "variable-primary",
            Self::NullableOrdinary => "nullable-ordinary",
            Self::UniqueManual => "unique-manual",
            Self::AliasHistory => "alias-history",
            Self::RelocatedPrimary => "relocated-primary",
            Self::RetainedTree => "retained-tree",
            Self::WrongCounterType => "wrong-counter-type",
            Self::SecondCounter => "second-counter",
            Self::UnknownCounterFlag => "unknown-counter-flag",
            Self::OrphanPhysical => "orphan-physical",
            Self::IncompatibleAlias => "incompatible-alias",
            Self::IgnoreNulls => "ignore-nulls",
            Self::SharedMapCell => "shared-map-cell",
            Self::IndexDataOverlap => "index-data-overlap",
            Self::FreeIndex => "free-index",
            Self::MissingOwned => "missing-owned",
            Self::UnreachableOwned => "unreachable-owned",
            Self::WrongKey => "wrong-key",
            Self::WrongRow => "wrong-row",
            Self::BadNodeBound => "bad-node-bound",
            Self::UnknownIndexHeader => "unknown-index-header",
            Self::InactiveIndexTail => "inactive-index-tail",
        }
    }
}

/// Explicit original facts, separate from any decoder's observations.
pub struct Source {
    /// Complete original image.
    pub bytes: Vec<u8>,
    /// Exact user table name.
    pub table: &'static str,
    /// Native column order, including readonly Id first.
    pub columns: Vec<String>,
    /// Original logical scalar rows, with NULL distinct from inactive bytes.
    pub rows: Vec<Vec<Value>>,
    /// Unindexed manual target: Score or Flag.
    pub target: &'static str,
    /// Target's native position.
    pub target_position: usize,
    /// Stored LAST for cases declaring a generator, including malformed controls.
    pub last_counter: Option<i32>,
    /// Historical physical index count, intentionally not recomputed.
    pub index_history: Option<u32>,
    /// Ordered logical index names; the two aliases share one physical slot.
    pub indexes: Vec<&'static str>,
    /// Physical index root when present.
    pub root: Option<u32>,
    /// Declared physical INDEX roles, regardless of adversarial ownership bits.
    pub index_pages: Vec<u32>,
    /// Allocated page roles; DATA8 and TD7 always remain in place.
    pub allocated: Vec<u32>,
    /// Explicit zero free holes.
    pub free: Vec<u32>,
    /// Original proposed ordinary-family disposition, never an authority token.
    pub ordinary: bool,
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    (0..count)
        .map(|slot| {
            let word = 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(page[word..word + 2].try_into().unwrap()));
            let end = if slot == 0 {
                PAGE
            } else {
                usize::from(u16::from_le_bytes(page[word - 2..word].try_into().unwrap()))
            };
            assert_eq!(start & 0xe000, 0);
            page[start..end].to_vec()
        })
        .collect()
}

fn packed(owner: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let records = rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>();
    let used = 14 + 2 * rows.len() + rows.iter().map(Vec::len).sum::<usize>();
    assert!(used <= PAGE);
    let mut page = data_page(owner, &records);
    put16(&mut page, 2, (PAGE - used) as u16);
    page
}

fn map(base: u32, bytes: usize, members: &[u32]) -> Vec<u8> {
    let mut row = vec![0; 5 + bytes];
    put32(&mut row, 1, base);
    for &member in members {
        let bit = usize::try_from(member.checked_sub(base).unwrap()).unwrap();
        assert!(bit < bytes * 8);
        row[5 + bit / 8] |= 1 << (bit % 8);
    }
    row
}

fn key(value: Option<i32>, slot: usize) -> Vec<u8> {
    let mut bytes = if let Some(value) = value {
        let mut bytes = vec![0x7f];
        bytes.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
        bytes
    } else {
        vec![0]
    };
    bytes.extend_from_slice(&((DATA << 8) | slot as u32).to_be_bytes());
    bytes
}

fn index_page(entries: &[Vec<u8>], node: bool, links: [u32; 3], prefix: usize) -> Vec<u8> {
    assert!(!entries.is_empty());
    let mut bytes = vec![0; PAGE];
    bytes[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    put32(&mut bytes, 4, DEFINITION);
    put32(&mut bytes, 12, links[0]);
    put32(&mut bytes, 16, links[1]);
    put32(&mut bytes, 20, links[2]);
    put16(&mut bytes, 24, prefix as u16);
    bytes[26] = u8::from(node);
    let mut end = 0;
    for (ordinal, entry) in entries.iter().enumerate() {
        let skipped = if ordinal == 0 { 0 } else { prefix };
        assert_eq!(&entry[..skipped], &entries[0][..skipped]);
        let suffix = &entry[skipped..];
        bytes[ENTRY_START + end..ENTRY_START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert!(end <= ENTRY_CAPACITY);
    put16(&mut bytes, 2, (ENTRY_CAPACITY - end) as u16);
    bytes
}

fn definition(
    original: &[u8],
    columns: usize,
    counter: Option<i32>,
    root: u32,
    aliases: usize,
    primary: bool,
    unique: bool,
    history: u32,
) -> (Vec<u8>, usize) {
    let old_length = u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize;
    assert_eq!(&original[47..55], &[0; 8]);
    assert_eq!(&original[old_length - 2..old_length], &[0xff, 0xff]);
    let mut active = original[..63].to_vec();
    active.extend_from_slice(&[0; 12]);
    put32(&mut active, 67, history);
    active.extend_from_slice(&original[63..old_length - 2]);
    if let Some(last) = counter {
        active[20..24].copy_from_slice(&last.to_le_bytes());
        active[75 + 15] = 7;
    }
    let physical_at = active.len();
    let mut physical = vec![0; 52];
    put32(&mut physical, 0, 1923);
    for number in 0..10 {
        put16(&mut physical, 4 + 3 * number, 0xffff);
    }
    put16(&mut physical, 4, 0);
    physical[6] = 1;
    put32(&mut physical, 34, (3 << 8) | 16);
    put32(&mut physical, 38, root);
    physical[46] = if primary {
        0x89
    } else if unique {
        0x81
    } else {
        0x80
    };
    active.extend_from_slice(&physical);
    for alias in 0..aliases {
        let mut logical = vec![0; 28];
        put32(&mut logical, 0, 1625);
        put32(&mut logical, 4, alias as u32);
        put32(&mut logical, 13, u32::MAX);
        logical[23] = u8::from(primary);
        active.extend_from_slice(&logical);
    }
    for name in ["IdKey", "IdAlias"].into_iter().take(aliases) {
        let raw = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        active.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        active.extend_from_slice(&raw);
    }
    active.extend_from_slice(&[0xff, 0xff]);
    let length = active.len();
    put32(&mut active, 8, length as u32);
    put32(&mut active, 47, aliases as u32);
    put32(&mut active, 51, 1);
    put16(&mut active, 2, (PAGE - length - 8) as u16);
    assert_eq!(
        u16::from_le_bytes(active[45..47].try_into().unwrap()) as usize,
        columns
    );
    assert!(length < PAGE);
    active.resize(PAGE, 0);
    (active, physical_at)
}

/// Author BEFORE only. Literal updates are a later independent acceptance input.
pub fn source(case: Case) -> Source {
    let variable = case == Case::VariablePrimary;
    let (mut bytes, columns, mut rows) = if variable {
        let seed = canonical_variable_creation::source(canonical_variable_creation::Case::Unicode);
        (
            seed.bytes,
            seed.columns
                .iter()
                .map(|column| column.name.to_owned())
                .collect(),
            seed.rows,
        )
    } else {
        let seed = canonical_creation::source(if case == Case::CounterOnly {
            canonical_creation::Case::One
        } else {
            canonical_creation::Case::Extrema
        });
        let rows = seed
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(Value::Int32).collect())
            .collect();
        (seed.bytes, seed.columns, rows)
    };
    let counter = match case {
        Case::CounterOnly | Case::RelocatedPrimary => Some(137_i32),
        Case::VariablePrimary => Some(0),
        Case::NullableOrdinary | Case::UniqueManual | Case::AliasHistory => None,
        _ => Some(-7),
    };
    let mut maps = records(&bytes[3 * PAGE..4 * PAGE]);
    assert_eq!(maps.len(), 16);
    let mut index_pages = Vec::new();
    let mut indexes = Vec::new();
    let mut root = None;
    let mut history = None;
    let mut free = Vec::new();

    if case == Case::CounterOnly {
        bytes[7 * PAGE + 20..7 * PAGE + 24].copy_from_slice(&137_i32.to_le_bytes());
        bytes[7 * PAGE + 63 + 15] = 7;
    } else {
        let aliases = if matches!(case, Case::AliasHistory | Case::IncompatibleAlias) {
            2
        } else {
            1
        };
        let primary = !matches!(
            case,
            Case::NullableOrdinary | Case::UniqueManual | Case::AliasHistory
        );
        let tree = matches!(case, Case::RetainedTree | Case::BadNodeBound);
        let root_page = if case == Case::RelocatedPrimary {
            95
        } else {
            9
        };
        index_pages = if tree {
            vec![9, 10, 11]
        } else {
            vec![root_page]
        };
        let pages = if case == Case::UnreachableOwned {
            11
        } else {
            index_pages.iter().copied().max().unwrap() as usize + 1
        };
        bytes.resize(pages * PAGE, 0);
        if case == Case::RelocatedPrimary {
            free = (9..95).collect();
        }
        let history_value = if case == Case::AliasHistory { 99 } else { 2 };
        let (mut td, physical) = definition(
            &bytes[7 * PAGE..8 * PAGE],
            columns.len(),
            counter,
            root_page,
            aliases,
            primary,
            case == Case::UniqueManual,
            history_value,
        );
        match case {
            Case::WrongCounterType => {
                td[75] = 3;
                put16(&mut td, 75 + 23, 2);
                for row in &mut rows {
                    let Value::Int32(value) = row[0] else {
                        panic!("the original counter control starts with Int32 Id");
                    };
                    row[0] = Value::Int16(value as i16);
                }
            }
            Case::SecondCounter => {
                td[75 + 25 + 15] = 7;
            }
            Case::UnknownCounterFlag => {
                td[75 + 15] |= 0x10;
            }
            Case::OrphanPhysical => {
                put32(&mut td, 47, 0);
            }
            Case::IncompatibleAlias => {
                td[physical + 52 + 28 + 23] = 2;
            }
            Case::IgnoreNulls => {
                td[physical + 46] |= 2;
            }
            Case::SharedMapCell => {
                put32(&mut td, physical + 34, (3 << 8) | 3);
            }
            _ => {}
        }
        bytes[7 * PAGE..8 * PAGE].copy_from_slice(&td);
        let mut entries = rows
            .iter()
            .enumerate()
            .map(|(slot, row)| {
                let value = match row[0] {
                    Value::Int32(value) => value,
                    Value::Int16(value) if case == Case::WrongCounterType => i32::from(value),
                    _ => panic!("literal original Id must have its declared integer carrier"),
                };
                key(Some(value), slot)
            })
            .collect::<Vec<_>>();
        if case == Case::NullableOrdinary {
            let data_rows = records(&bytes[8 * PAGE..9 * PAGE]);
            let mut changed = data_rows;
            let end = changed[1].len();
            assert_eq!(changed[1][end - 1] & 1, 1);
            changed[1][end - 1] &= !1;
            bytes[8 * PAGE..9 * PAGE].copy_from_slice(&packed(DEFINITION, &changed));
            rows[1][0] = Value::Null;
            entries[1] = key(None, 1);
        }
        if case == Case::WrongKey {
            entries[0] = key(Some(9), 0);
        }
        if case == Case::WrongRow {
            entries[0] = key(Some(1), 1);
        }
        entries.sort();
        if tree {
            let mut bound = if case == Case::BadNodeBound {
                key(Some(0), 0)
            } else {
                entries[0].clone()
            };
            bound.extend_from_slice(&10_u32.to_be_bytes());
            bytes[9 * PAGE..10 * PAGE].copy_from_slice(&index_page(&[bound], true, [0, 0, 11], 4));
            bytes[10 * PAGE..11 * PAGE].copy_from_slice(&index_page(
                &entries[..1],
                false,
                [0, 11, 0],
                4,
            ));
            bytes[11 * PAGE..12 * PAGE].copy_from_slice(&index_page(
                &entries[1..],
                false,
                [10, 0, 0],
                4,
            ));
        } else {
            bytes[root_page as usize * PAGE..(root_page as usize + 1) * PAGE]
                .copy_from_slice(&index_page(&entries, false, [0, 0, 0], 0));
        }
        let members = match case {
            Case::IndexDataOverlap => vec![8, root_page],
            Case::MissingOwned => Vec::new(),
            Case::UnreachableOwned => vec![9, 10],
            _ => index_pages.clone(),
        };
        maps.push(if case == Case::RelocatedPrimary {
            map(64, 4, &members)
        } else {
            map(0, 8, &members)
        });
        if case == Case::UnknownIndexHeader {
            bytes[9 * PAGE + 8] = 1;
        }
        if case == Case::InactiveIndexTail {
            assert_eq!(entries.len(), 2);
            assert!(entries.iter().all(|entry| entry.len() == 9));
            bytes[9 * PAGE + ENTRY_START + 18..10 * PAGE].fill(0xa7);
        }
        indexes.push("IdKey");
        if aliases == 2 {
            indexes.push("IdAlias");
        }
        root = Some(root_page);
        history = Some(history_value);
    }
    bytes[3 * PAGE..4 * PAGE].copy_from_slice(&packed(0, &maps));
    let mut global = map(0, if bytes.len() > 64 * PAGE { 12 } else { 8 }, &free);
    if case == Case::FreeIndex {
        global[6] |= 2;
    }
    bytes[PAGE..2 * PAGE].copy_from_slice(&packed(1, &[global]));
    let allocated = (0..(bytes.len() / PAGE) as u32)
        .filter(|page| !free.contains(page))
        .collect();
    Source {
        bytes,
        table: "Items",
        columns,
        rows,
        target: if variable { "Flag" } else { "Score" },
        target_position: if variable { 4 } else { 1 },
        last_counter: counter,
        index_history: history,
        indexes,
        root,
        index_pages,
        allocated,
        free,
        ordinary: matches!(
            case,
            Case::CounterOnly
                | Case::CounterPrimary
                | Case::VariablePrimary
                | Case::NullableOrdinary
                | Case::UniqueManual
                | Case::AliasHistory
                | Case::RelocatedPrimary
                | Case::RetainedTree
                | Case::InactiveIndexTail
        ),
    }
}
