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
