use super::check_table_long_values;
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{data_page, mutation_fixture};

fn forwarded() -> Vec<u8> {
    let mut bytes = mutation_fixture::with_rows(&[(7, 42)]);
    let terminal = 40usize;
    bytes.resize((terminal + 1) * 4096, 0);
    for page in 12..terminal {
        let pointer = (((page + 1) as u32) << 8).to_le_bytes().to_vec();
        let raw = data_page(10, &[(0x4000, pointer)]);
        bytes[page * 4096..(page + 1) * 4096].copy_from_slice(&raw);
    }
    let mut row = 2_u16.to_le_bytes().to_vec();
    row.extend(7_i32.to_le_bytes());
    row.extend(42_i32.to_le_bytes());
    row.push(3);
    let raw = data_page(10, &[(0x8000, row)]);
    bytes[terminal * 4096..(terminal + 1) * 4096].copy_from_slice(&raw);
    bytes
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 1000, 1_000_000)
}

#[test]
fn public_consistency_read_follows_long_rows_without_a_mutation_hop_quota() {
    let bytes = forwarded();
    let report = check_table_long_values(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), 1);
    assert!(report.columns().is_empty());
}

#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
#[test]
fn mutation_consistency_purpose_retains_the_existing_physical_hop_scope() {
    let bytes = forwarded();
    assert!(matches!(
        super::check_for_mutation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        ),
        Err(crate::InspectError::StructureLimit)
    ));
}

fn many_rows() -> Vec<u8> {
    let count = 16_385usize;
    let pages = count.div_ceil(250);
    let mut bytes = mutation_fixture::with_rows(&[]);
    bytes.resize((12 + pages) * 4096, 0);
    bytes[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&(count as u32).to_le_bytes());
    let mut bitmap = vec![0; 5 + (12 + pages).div_ceil(8)];
    for index in 0..pages {
        let number = 12 + index;
        bitmap[5 + number / 8] |= 1 << (number % 8);
        let rows = (0..(count - index * 250).min(250))
            .map(|row| {
                let mut data = 2_u16.to_le_bytes().to_vec();
                data.extend(((index * 250 + row) as i32).to_le_bytes());
                data.extend(42_i32.to_le_bytes());
                data.push(3);
                (0, data)
            })
            .collect::<Vec<_>>();
        bytes[number * 4096..(number + 1) * 4096].copy_from_slice(&data_page(10, &rows));
    }
    bytes[11 * 4096..12 * 4096].copy_from_slice(&data_page(1, &[(0, bitmap)]));
    bytes
}

#[test]
fn public_consistency_read_counts_all_rows_and_honors_an_explicit_row_allowance() {
    let bytes = many_rows();
    let report = check_table_long_values(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(64 * 1024 * 1024, 20_000, 10_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), 16_385);
    assert!(matches!(
        check_table_long_values(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(64 * 1024 * 1024, 100, 10_000_000),
            &|| false,
        ),
        Err(crate::InspectError::Limit(_))
    ));
    #[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
    assert!(matches!(
        super::check_for_mutation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(64 * 1024 * 1024, 20_000, 10_000_000),
            &|| false,
        ),
        Err(crate::InspectError::StructureLimit)
    ));

    let mut inconsistent = bytes;
    inconsistent[10 * 4096 + 16..10 * 4096 + 20].copy_from_slice(&16_384_u32.to_le_bytes());
    assert!(matches!(
        check_table_long_values(
            &mut Cursor::new(&inconsistent),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(64 * 1024 * 1024, 20_000, 10_000_000),
            &|| false,
        ),
        Err(crate::InspectError::Malformed)
    ));
}
