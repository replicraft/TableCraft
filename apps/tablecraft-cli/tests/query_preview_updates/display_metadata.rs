//! One existing C4/V2 source supports literal text UPDATE and DELETE images.
#[path = "display_metadata/api.rs"]
mod api;

use super::*;

const PAGE_BYTES: usize = 4096;

fn display_case() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut before = source::source();
    before[20] = 6;
    let property_source = fixture::with_rule("Format", ">");
    let mut catalog = property_source[4 * PAGE_BYTES..5 * PAGE_BYTES].to_vec();
    // The existing literal property owner binds Score. Label has the same five
    // UTF16 units; replace only that exact unique scope name in this synthetic
    // catalog source, without copying the MR2 encoder or changing its framing.
    let old = [83, 0, 99, 0, 111, 0, 114, 0, 101, 0];
    let at = catalog
        .windows(old.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == old).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(at.len(), 1);
    catalog[at[0]..at[0] + old.len()].copy_from_slice(&[76, 0, 97, 0, 98, 0, 101, 0, 108, 0]);
    before[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&catalog);

    let mut updated = before.clone();
    let word = u16::from_le_bytes(
        before[12 * PAGE_BYTES + 14..12 * PAGE_BYTES + 16]
            .try_into()
            .unwrap(),
    );
    let label = 12 * PAGE_BYTES + usize::from(word & 0x1fff) + 10;
    assert_eq!(&before[label..label + 2], &[65, 0]);
    updated[label..label + 2].copy_from_slice(&[109, 0]);

    let mut deleted = before.clone();
    let slot = 12 * PAGE_BYTES + 16;
    let word = u16::from_le_bytes(before[slot..slot + 2].try_into().unwrap());
    deleted[slot..slot + 2].copy_from_slice(&(word | 0xc000).to_le_bytes());
    deleted[10 * PAGE_BYTES + 16..10 * PAGE_BYTES + 20].copy_from_slice(&3_u32.to_le_bytes());
    (before, updated, deleted)
}

fn checked_display(bytes: &[u8], count: usize) {
    let table = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), count);
    assert!(table.indexes().is_empty());
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), count);
    assert_ne!(
        report.table_validation(),
        tablecraft_ace::RuleCheckStatus::Unsupported
    );
    assert!(report.fields().iter().all(|field| {
        ![field.required(), field.empty_text(), field.validation()]
            .contains(&tablecraft_ace::RuleCheckStatus::Unsupported)
    }));
}

#[test]
fn qualify_cli_text_display_source_and_complete_update_delete_expected_before_actions() {
    let (before, updated, deleted) = display_case();
    checked_display(&before, 4);
    checked_display(&updated, 4);
    checked_display(&deleted, 3);
    assert_eq!(rows(&updated)[0][1], Value::Text("m".to_owned()));
    assert_eq!(
        rows(&deleted)
            .iter()
            .map(|row| row[0].clone())
            .collect::<Vec<_>>(),
        [Value::Int32(1), Value::Int32(3), Value::Int32(4)]
    );
    for candidate in [&updated, &deleted] {
        assert_eq!(
            &candidate[4 * PAGE_BYTES..5 * PAGE_BYTES],
            &before[4 * PAGE_BYTES..5 * PAGE_BYTES]
        );
    }
    let properties = tablecraft_ace::inspect_table_properties(
        &mut Cursor::new(&before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "Label")
        .unwrap();
    let format = group.get("Format").unwrap();
    assert_eq!(format.native_type(), 10);
    assert_eq!(format.flags(), 0);
    assert_eq!(format.text().unwrap(), ">");
}
