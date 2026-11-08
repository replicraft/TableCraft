//! Raw mixed two/three-page ledgers composed from independent native sources.
//! No production creator, planner, schema decoder or intent encoder is called.
use super::{
    canonical_creation as fixed, canonical_creation_autonumber as generated,
    canonical_creation_integral_primary as wide, canonical_creation_primary as primary,
    canonical_native17_creation as native17, put16, put32,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const PHYSICAL_INDEX: usize = 75 + 2 * 25 + 6 + 12;
const OPAQUE_AT: usize = 3500;
const DATA_GAP: usize = 512;

#[derive(Clone, Copy, Debug)]
pub enum Family {
    Keyless,
    Int32,
    Int64,
    Currency,
    Generated,
}

impl Family {
    pub const fn keyed(self) -> bool {
        !matches!(self, Self::Keyless)
    }

    pub const fn pages(self) -> usize {
        match self {
            Self::Keyless => 2,
            Self::Int32 | Self::Int64 | Self::Currency | Self::Generated => 3,
        }
    }
}

pub struct Source {
    pub bytes: Vec<u8>,
    pub first_rows: Vec<Vec<Value>>,
}

fn template(family: Family, populated: bool) -> Source {
    match family {
        Family::Keyless => {
            let original = fixed::source(if populated {
                fixed::Case::One
            } else {
                fixed::Case::Empty
            });
            Source {
                bytes: native17::source(original.bytes),
                first_rows: original
                    .rows
                    .into_iter()
                    .map(|row| row.into_iter().map(Value::Int32).collect())
                    .collect(),
            }
        }
        Family::Int32 => {
            let original = primary::source(if populated {
                primary::Case::One
            } else {
                primary::Case::Empty
            });
            Source {
                bytes: original.bytes,
                first_rows: original.rows,
            }
        }
        Family::Int64 | Family::Currency => {
            let original = wide::source(wide::Case {
                family: if matches!(family, Family::Int64) {
                    wide::Family::Int64
                } else {
                    wide::Family::Currency
                },
                scenario: if populated {
                    wide::Scenario::SignedUnsorted
                } else {
                    wide::Scenario::Empty
                },
            });
            Source {
                bytes: original.bytes,
                first_rows: original.rows,
            }
        }
        Family::Generated => {
            let original = generated::source(if populated {
                generated::Case::HighLast
            } else {
                generated::Case::EmptyPrimary
            });
            Source {
                bytes: original.bytes,
                first_rows: original.rows,
            }
        }
    }
}

/// First Items table is populated; later MixedN tables are explicitly empty.
/// All old TD/DATA/root opaque sentinels remain outside the append envelope.
pub fn source(families: &[Family]) -> Source {
    assert!(!families.is_empty());
    let mut original = template(families[0], true);
    original.bytes[7 * PAGE + OPAQUE_AT] = 0xb3;
    original.bytes[8 * PAGE + DATA_GAP] = 0x6d;
    if families[0].keyed() {
        original.bytes[9 * PAGE + OPAQUE_AT] = 0xc7;
    }
    let mut user_pages = families[0].pages();
    for (position, family) in families.iter().copied().enumerate().skip(1) {
        append(&mut original.bytes, position, user_pages, family);
        user_pages += family.pages();
    }
    assert_eq!(original.bytes.len(), (7 + user_pages) * PAGE);
    original
}

/// One keyed table plus twenty-six keyless tables occupies 62 pages. The last
/// two bitmap positions admit a keyless table, while another root cannot fit.
pub fn capacity_source() -> Source {
    let mut families = vec![Family::Generated];
    families.extend([Family::Keyless; 26]);
    source(&families)
}

fn append(bytes: &mut Vec<u8>, position: usize, user_pages: usize, family: Family) {
    let definition_page = 7 + user_pages;
    assert!(definition_page + family.pages() <= 64);
    let data_page = definition_page + 1;
    let owned = 20 + user_pages;
    let mut members = vec![data_page, data_page];
    if family.keyed() {
        members.push(definition_page + 2);
    }
    for member in members {
        let mut map = [0; 13];
        map[5 + member / 8] = 1 << (member % 8);
        append_record(&mut bytes[3 * PAGE..4 * PAGE], &map);
    }
    let catalog = fixed::catalog_record(
        definition_page as u32,
        0x0f00_0001,
        &format!("Mixed{}", position + 1),
        1,
        0,
    );
    append_record(
        &mut bytes[4 * PAGE..5 * PAGE],
        &native17::catalog_row(&catalog),
    );
    put32(bytes, 2 * PAGE + 16, (8 + position) as u32);

    let template = template(family, false).bytes;
    let mut definition = template[7 * PAGE..8 * PAGE].to_vec();
    put32(&mut definition, 55, (3 << 8) | owned as u32);
    put32(&mut definition, 59, (3 << 8) | (owned + 1) as u32);
    if family.keyed() {
        put32(
            &mut definition,
            PHYSICAL_INDEX + 34,
            (3 << 8) | (owned + 2) as u32,
        );
        put32(
            &mut definition,
            PHYSICAL_INDEX + 38,
            (definition_page + 2) as u32,
        );
    }
    bytes.extend(definition);
    let mut data = template[8 * PAGE..9 * PAGE].to_vec();
    put32(&mut data, 4, definition_page as u32);
    bytes.extend(data);
    if family.keyed() {
        let mut root = template[9 * PAGE..10 * PAGE].to_vec();
        put32(&mut root, 4, definition_page as u32);
        bytes.extend(root);
    }
}

fn append_record(page: &mut [u8], row: &[u8]) {
    let count = u16::from_le_bytes(page[12..14].try_into().unwrap()) as usize;
    let free = u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize;
    let slot = 14 + count * 2;
    let end = slot + free;
    let start = end.checked_sub(row.len()).unwrap();
    assert!(start >= slot + 2);
    assert!(page[slot..slot + 2].iter().all(|byte| *byte == 0));
    assert!(page[start..end].iter().all(|byte| *byte == 0));
    page[start..end].copy_from_slice(row);
    put16(page, slot, start as u16);
    put16(page, 12, (count + 1) as u16);
    put16(page, 2, (free - row.len() - 2) as u16);
}
