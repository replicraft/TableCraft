//! Actual catalog member-role checks on unchanged pinned native sources.
use super::require;
use crate::{
    NativeImage,
    format::{allocation, bytes::u32_at, reader::row_range},
    inspect_table_indexes,
    table::mutation::{lval_ownership::Inventory, map_authority},
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_retained_numeric::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(137);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(digest, expected);
    })
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn check(bytes: &[u8], definition: u32) -> Result<(), crate::EditError> {
    let image = image(bytes);
    let live = Inventory::capture(&image, &mut budget(), &|| false)?;
    require(&image, definition, &live, &mut budget(), &|| false)
}

fn field32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
#[ignore = "requires both pinned original files and all eight literal outcomes"]
fn actual_retained_index_member_roles_allow_only_canonical_physical_aliases() {
    for case in cases() {
        let original = image(&case.before);
        let indexes = inspect_table_indexes(
            &mut original.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexes.indexes().len(), 4);
        check(&case.before, indexes.schema().definition_page()).unwrap();
        assert_eq!(original.as_bytes(), case.before);
    }
}

#[test]
#[ignore = "requires both pinned original files and all eight literal outcomes"]
fn index_members_cannot_share_other_physical_index_data_or_metadata_roles() {
    for case in cases().into_iter().filter(|case| case.table == "ByteKeys") {
        let original = image(&case.before);
        let indexes = inspect_table_indexes(
            &mut original.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let definition = indexes.schema().definition_page();
        let td = definition as usize * PAGE;
        let columns =
            u16::from_le_bytes(case.before[td + 45..td + 47].try_into().unwrap()) as usize;
        let physical = field32(&case.before, td + 51) as usize;
        let mut index_start = td + 63 + physical * 12 + columns * 25;
        for _ in 0..columns {
            let length = u16::from_le_bytes(
                case.before[index_start..index_start + 2]
                    .try_into()
                    .unwrap(),
            ) as usize;
            index_start += 2 + length;
        }
        assert!(physical >= 2);
        // Independent descriptor offsets: physical records52B, map at34,
        // root at38. Distinct data numbers must not alias their member set.
        let first_map = field32(&case.before, index_start + 34);
        let first_root = field32(&case.before, index_start + 38);
        let second = index_start + 52;
        for variant in 0..3 {
            let mut bytes = case.before.clone();
            match variant {
                0 => {
                    put32(&mut bytes, second + 34, first_map);
                    put32(&mut bytes, second + 38, first_root);
                }
                1 => {
                    let data_map = field32(&case.before, td + 55);
                    put32(&mut bytes, second + 34, data_map);
                }
                2 => {
                    let map_page = (first_map >> 8) as usize;
                    let slot = (first_map & 255) as usize;
                    let page = &case.before[map_page * PAGE..(map_page + 1) * PAGE];
                    let (_, start, end) = row_range(page, slot).unwrap();
                    assert_eq!(page[start], 0);
                    let base = u32_at(page, start + 1).unwrap() as usize;
                    let mut rebased = vec![0_u8; end - start - 5];
                    for (offset, byte) in page[start + 5..end].iter().enumerate() {
                        for bit in 0..8 {
                            if byte & (1 << bit) != 0 {
                                let number = base + offset * 8 + bit;
                                assert!(number / 8 < rebased.len());
                                rebased[number / 8] |= 1 << (number % 8);
                            }
                        }
                    }
                    // Retain every actual owned member while making page0
                    // representable, then add only that metadata collision.
                    rebased[0] |= 1;
                    let absolute = map_page * PAGE + start;
                    put32(&mut bytes, absolute + 1, 0);
                    bytes[absolute + 5..map_page * PAGE + end].copy_from_slice(&rebased);
                }
                _ => unreachable!(),
            }
            assert!(
                check(&bytes, definition).is_err(),
                "h{} variant {variant}",
                case.header
            );
        }
    }
}

#[test]
#[ignore = "requires both pinned original files and all eight literal outcomes"]
fn selected_index_members_cannot_be_column_owned_or_live_lval_pages() {
    for case in cases().into_iter().filter(|case| case.table == "ByteKeys") {
        let original = image(&case.before);
        let indexes = inspect_table_indexes(
            &mut original.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let definition = indexes.schema().definition_page();
        let selected = indexes
            .indexes()
            .iter()
            .find(|index| index.data_number() == 1)
            .unwrap();
        let mut lval_map = None;
        map_authority::capture_catalog(
            &original,
            &mut budget(),
            &|| false,
            |_schema, _indexes, declarations, reader| {
                for declaration in &declarations.entries {
                    reader.tick()?;
                    if lval_map.is_none()
                        && !allocation::pages(reader, declaration.references.owned())?.is_empty()
                    {
                        lval_map = Some(declaration.references.owned());
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        let lval_map = lval_map.expect("the unchanged fixture retains nonempty Memo/OLE ownership");
        let mut bytes = case.before.clone();
        let td = definition as usize * PAGE;
        let count = u16::from_le_bytes(bytes[td + 45..td + 47].try_into().unwrap()) as usize;
        let physical = field32(&bytes, td + 51) as usize;
        let mut at = td + 63 + physical * 12 + count * 25;
        for _ in 0..count {
            let length = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize;
            at += 2 + length;
        }
        put32(
            &mut bytes,
            at + selected.data_number() as usize * 52 + 34,
            lval_map,
        );
        assert!(check(&bytes, definition).is_err());
    }
}
