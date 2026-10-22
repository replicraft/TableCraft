use super::{fixture, support};
use std::{io::Cursor, ops::Bound::Included};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, InspectError, Int32IndexSelection,
    read_table_index, read_table_index_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DATA_START: usize = 15;
const DATA_PAGES: usize = 100;
const ROWS_PER_PAGE: usize = 200;
const LEAF_START: usize = DATA_START + DATA_PAGES;
const LEAVES: usize = 50;
const KEYS_PER_LEAF: usize = 400;
const ROWS: usize = DATA_PAGES * ROWS_PER_PAGE;
const INDEX_ROOT: usize = 14;
const TABLE_DEFINITION: usize = 10;
const TABLE_USAGE: usize = 11;
const INDEX_USAGE: usize = 13;
const ENTRY_START: usize = 480;
const ENTRY_MASK: usize = 27;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

fn database(code: u8, descending: bool) -> Vec<u8> {
    let mut bytes = fixture::database(
        code,
        &[4, 4],
        &[(0, descending)],
        &[vec![Some(1), Some(10)]],
        0,
    );
    if code == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes.resize((LEAF_START + LEAVES) * PAGE_BYTES, 0);
    support::put32(&mut bytes, TABLE_DEFINITION * PAGE_BYTES + 16, ROWS as u32);
    for (usage, owned) in [
        (TABLE_USAGE, (DATA_START..LEAF_START).collect::<Vec<_>>()),
        (
            INDEX_USAGE,
            [INDEX_ROOT]
                .into_iter()
                .chain(LEAF_START..LEAF_START + LEAVES)
                .collect(),
        ),
    ] {
        let mut map = vec![0; 5 + (LEAF_START + LEAVES).div_ceil(8)];
        for number in owned {
            map[5 + number / 8] |= 1 << (number % 8);
        }
        bytes[usage * PAGE_BYTES..(usage + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
    }
    for page in DATA_START..LEAF_START {
        let rows = (0..ROWS_PER_PAGE)
            .map(|slot| {
                let id = ((page - DATA_START) * ROWS_PER_PAGE + slot + 1) as i32;
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(id.to_le_bytes());
                row.extend((id * 10).to_le_bytes());
                row.push(3);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(TABLE_DEFINITION as u32, &rows));
    }
    let entry = |ordinal: usize| {
        let position = if descending {
            ROWS - ordinal - 1
        } else {
            ordinal
        };
        let id = (position + 1) as u32;
        let mut entry = vec![if descending { 0x80 } else { 0x7f }];
        entry.extend(
            (id ^ 0x80000000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
        let page = (DATA_START + position / ROWS_PER_PAGE) as u32;
        let slot = (position % ROWS_PER_PAGE) as u32;
        entry.extend(((page << 8) | slot).to_be_bytes());
        entry
    };
    for number in [INDEX_ROOT]
        .into_iter()
        .chain(LEAF_START..LEAF_START + LEAVES)
    {
        let leaf = number != INDEX_ROOT;
        let page = &mut bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES];
        page.fill(0);
        page[..2].copy_from_slice(&[if leaf { 4 } else { 3 }, 1]);
        support::put32(page, 4, TABLE_DEFINITION as u32);
        if leaf {
            support::put32(
                page,
                12,
                if number == LEAF_START {
                    0
                } else {
                    (number - 1) as u32
                },
            );
            support::put32(
                page,
                16,
                if number + 1 == LEAF_START + LEAVES {
                    0
                } else {
                    (number + 1) as u32
                },
            );
        }
        page[26] = u8::from(!leaf);
        let mut end = 0;
        let count = if leaf { KEYS_PER_LEAF } else { LEAVES };
        for position in 0..count {
            let ordinal = if leaf {
                (number - LEAF_START) * KEYS_PER_LEAF + position
            } else {
                (position + 1) * KEYS_PER_LEAF - 1
            };
            let mut key = entry(ordinal);
            if !leaf {
                key.extend(((LEAF_START + position) as u32).to_be_bytes());
            }
            page[ENTRY_START + end..ENTRY_START + end + key.len()].copy_from_slice(&key);
            end += key.len();
            page[ENTRY_MASK + end / 8] |= 1 << (end % 8);
        }
        support::put16(page, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    }
    bytes
}

fn read(
    bytes: &[u8],
    maximum_rows: u32,
    columns: Option<&[&str]>,
    selection: IndexProjectionSelection<'_>,
) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns,
            selection,
            maximum_rows,
        },
        &mut budget(),
        &|| false,
    )
}

#[test]
fn large_complete_and_projected_indexes_match_independent_rows_in_both_directions() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let bytes = database(code, descending);
            let table = read(&bytes, 20_000, None, IndexProjectionSelection::All).unwrap();
            let expected = (1..=20_000)
                .map(|position| {
                    let id = if descending {
                        20_001 - position
                    } else {
                        position
                    };
                    vec![Value::Int32(id), Value::Int32(id * 10)]
                })
                .collect::<Vec<_>>();
            assert_eq!(table.rows(), expected);
            let projected = read(
                &bytes,
                20_000,
                Some(&["Score"]),
                IndexProjectionSelection::All,
            )
            .unwrap();
            assert_eq!(
                projected.rows(),
                expected
                    .iter()
                    .map(|row| vec![row[1].clone()])
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                read(&bytes, 20_000, Some(&[]), IndexProjectionSelection::All)
                    .unwrap()
                    .rows(),
                vec![Vec::<Value>::new(); 20_000]
            );
            assert_eq!(
                read(&bytes, 19_999, None, IndexProjectionSelection::All).err(),
                Some(InspectError::StructureLimit)
            );
            assert_eq!(
                read_table_index(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::StructureLimit)
            );
        }
    }
}

#[test]
fn small_ranges_in_large_indexes_limit_matches_and_check_hidden_keys() {
    for code in [1, 2, 5, 6] {
        for descending in [false, true] {
            let mut bytes = database(code, descending);
            let selection = IndexProjectionSelection::Int32(Int32IndexSelection::Range {
                lower: Included(19_998),
                upper: Included(20_000),
            });
            let table = read(&bytes, 3, Some(&["Score"]), selection).unwrap();
            let mut expected = vec![
                vec![Value::Int32(199_980)],
                vec![Value::Int32(199_990)],
                vec![Value::Int32(200_000)],
            ];
            if descending {
                expected.reverse();
            }
            assert_eq!(table.rows(), expected);
            assert_eq!(
                read(&bytes, 2, Some(&[]), selection).err(),
                Some(InspectError::StructureLimit)
            );
            let page = (LEAF_START - 1) * PAGE_BYTES;
            let slot = ROWS_PER_PAGE - 1;
            let offset = u16::from_le_bytes(
                bytes[page + 14 + 2 * slot..page + 16 + 2 * slot]
                    .try_into()
                    .unwrap(),
            ) as usize;
            support::put32(&mut bytes, page + offset + 2, 12_345);
            assert_eq!(
                read(&bytes, 3, Some(&["Score"]), selection).err(),
                Some(InspectError::Malformed)
            );
        }
    }
}
