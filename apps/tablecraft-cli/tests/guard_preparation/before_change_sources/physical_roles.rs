//! Additive physically complete derivative; original read-only fixtures stay exact.
use super::super::support;
use super::{CASES, expected, model, qualify_view};

const PAGE_BYTES: usize = 4096;
const GLOBAL_PAGE: usize = 1;
const OWNER_OFFSET: usize = 4;
const DEFINITION_NAMES_OFFSET: usize = 63;
const COLUMN_BYTES: usize = 25;
const OWNED_MAPS: [(usize, &[usize]); 3] = [(3, &[4]), (8, &[]), (11, &[12])];

pub(super) fn source(code: u8, body: &str) -> (Vec<u8>, String, Vec<u8>) {
    let (mut bytes, xml, bag) = model::source(code, "BeforeChange", body);
    assert_eq!(bytes.len(), 14 * PAGE_BYTES);
    assert!(
        bytes[PAGE_BYTES..2 * PAGE_BYTES]
            .iter()
            .all(|byte| *byte == 0)
    );
    // Literal inline GLOBAL-free map: start0, two zero bitmap bytes. Every
    // represented source page is allocated; no future free/owned claims exist.
    bytes[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&support::data_page(1, &[(0, vec![0; 7])]));
    for (page, _) in OWNED_MAPS {
        let offset = page * PAGE_BYTES + OWNER_OFFSET;
        assert_eq!(&bytes[offset..offset + 4], &1_u32.to_le_bytes());
        bytes[offset..offset + 4].copy_from_slice(&0_u32.to_le_bytes());
    }
    for (page, columns, terminal) in [(2, 9, 398), (7, 8, 449), (10, 2, 131)] {
        let definition = &mut bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES];
        assert_eq!(&definition[..2], &[2, 1]);
        assert_eq!(
            u16::from_le_bytes(definition[45..47].try_into().unwrap()),
            columns
        );
        let mut name = DEFINITION_NAMES_OFFSET + COLUMN_BYTES * usize::from(columns);
        for _ in 0..columns {
            let length = usize::from(u16::from_le_bytes(
                definition[name..name + 2].try_into().unwrap(),
            ));
            assert_eq!(length % 2, 0);
            name += 2 + length;
        }
        assert_eq!(name, terminal);
        assert_eq!(&definition[name..name + 2], &[0, 0]);
        definition[name..name + 2].copy_from_slice(&[255, 255]);
    }
    (bytes, xml, bag)
}

fn inline_map(bytes: &[u8], page: usize) -> &[u8] {
    let data = &bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES];
    assert_eq!(&data[..2], &[1, 1]);
    assert_eq!(
        &data[OWNER_OFFSET..OWNER_OFFSET + 4],
        &(if page == GLOBAL_PAGE { 1_u32 } else { 0_u32 }).to_le_bytes()
    );
    assert_eq!(&data[12..14], &1_u16.to_le_bytes());
    let start = usize::from(u16::from_le_bytes(data[14..16].try_into().unwrap()));
    assert_eq!(start & 0xe000, 0);
    let record = &data[start..];
    assert_eq!(&record[..5], &[0; 5]);
    record
}

fn memberships(record: &[u8]) -> Vec<usize> {
    record[5..]
        .iter()
        .enumerate()
        .flat_map(|(byte, bits)| {
            (0..8).filter_map(move |bit| (bits & (1 << bit) != 0).then_some(byte * 8 + bit))
        })
        .collect()
}

#[test]
fn qualify_before_change_sql_complete_physical_roles_before_service() {
    let mut pairs = 0;
    for code in [5, 6] {
        for (_, body, score) in CASES {
            let (before, _, bag) = source(code, body);
            let after = expected(&before, score);
            for (bytes, value) in [(&before, 20), (&after, score.unwrap_or(20))] {
                let global = inline_map(bytes, GLOBAL_PAGE);
                assert!(memberships(global).is_empty());
                assert!(global.len() >= 7);
                for page in 0..14 {
                    assert_eq!(global[5 + page / 8] & (1 << (page % 8)), 0);
                }
                for (page, members) in OWNED_MAPS {
                    assert_eq!(memberships(inline_map(bytes, page)), members);
                }
                for (page, terminal) in [(2, 398), (7, 449), (10, 131)] {
                    assert_eq!(
                        &bytes[page * PAGE_BYTES + terminal..page * PAGE_BYTES + terminal + 2],
                        &[255, 255]
                    );
                }
                qualify_view(bytes, value, &bag);
            }
            assert_eq!(
                &before[PAGE_BYTES..2 * PAGE_BYTES],
                &after[PAGE_BYTES..2 * PAGE_BYTES]
            );
            pairs += 1;
        }
    }
    assert_eq!(pairs, 8);
}
