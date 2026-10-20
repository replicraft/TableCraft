use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    GeneralSortFamily, InspectError, inspect_database_sort_order, inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 10_000, 1_000_000)
}
fn header(code: u8, order: [u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0; 3 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(if code == 1 {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    });
    bytes[20] = code;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    // Independently specified root-mask bytes for offsets 110..114.
    for (i, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + i] = order[i] ^ mask;
    }
    bytes
}
#[test]
fn all_read_profiles_preserve_lcid_variant_version_and_unknown_orders() {
    for code in [1, 2, 3, 5, 6] {
        for raw in [[9, 4, 0, 0], [9, 4, 0, 1], [31, 4, 2, 7], [0, 0, 0, 0]] {
            let bytes = header(code, raw);
            let order =
                inspect_database_sort_order(&mut Cursor::new(&bytes), &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(order.lcid(), u16::from_le_bytes([raw[0], raw[1]]));
            assert_eq!(order.variant(), raw[2]);
            assert_eq!(order.version(), raw[3]);
            assert_eq!(
                order.general_family(),
                match raw {
                    [9, 4, 0, 0] => Some(GeneralSortFamily::Legacy),
                    [9, 4, 0, 1] => Some(GeneralSortFamily::General2010),
                    _ => None,
                }
            );
        }
    }
}
#[test]
fn encoded_headers_are_inert_and_failures_do_not_publish_guessed_settings() {
    let mut bytes = header(5, [9, 4, 0, 1]);
    bytes[62] ^= 1;
    let mut source = Cursor::new(&bytes);
    assert_eq!(
        inspect_database_sort_order(&mut source, &mut budget(), &|| false)
            .unwrap()
            .general_family(),
        Some(GeneralSortFamily::General2010)
    );
    assert!(matches!(
        inspect_database_sort_order(&mut source, &mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(InspectError::Limit(_))
    ));
    let visits = Cell::new(0);
    inspect_database_sort_order(&mut source, &mut budget(), &|| {
        visits.set(visits.get() + 1);
        false
    })
    .unwrap();
    let last = visits.get();
    visits.set(0);
    assert_eq!(
        inspect_database_sort_order(&mut source, &mut budget(), &|| {
            visits.set(visits.get() + 1);
            visits.get() == last
        }),
        Err(InspectError::Cancelled)
    );
    assert_eq!(source.into_inner(), &bytes);
}
#[test]
fn column_descriptors_remain_distinct_from_database_defaults_and_numeric_metadata() {
    let mut bytes = include_bytes!("../../../../docs/examples/Access_Example_VBA.accdb").to_vec();
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(schema.columns()[0].text_sort_order().is_none());
    assert_eq!(
        schema.columns()[1]
            .text_sort_order()
            .unwrap()
            .general_family(),
        Some(GeneralSortFamily::Legacy)
    );
    let page = schema.definition_page() as usize * 4096;
    let indexes = u32::from_le_bytes(bytes[page + 51..page + 55].try_into().unwrap()) as usize;
    let offset = page + 63 + indexes * 12 + 25 + 11;
    for descriptor in [[0, 0, 0, 0], [9, 4, 1, 1], [31, 4, 0, 1]] {
        bytes[offset..offset + 4].copy_from_slice(&descriptor);
        let schema = inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let order = schema.columns()[1].text_sort_order().unwrap();
        assert_eq!(order.general_family(), None);
        assert_eq!(order.variant(), descriptor[2]);
        assert!(schema.columns()[0].text_sort_order().is_none());
    }
}
