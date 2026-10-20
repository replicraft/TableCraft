use super::*;
#[test]
fn bounded_pages_work_above_materialized_map_limits_in_all_read_profiles() {
    for code in [1, 2, 5, 6] {
        for indirect in [false, true] {
            let mut bytes = map_fixture::database(6000, indirect, false);
            bytes[20] = code;
            if code != 1 {
                bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            }
            let mut input = PageReads::new(bytes.clone());
            let result = read_table_page(
                &mut input,
                SCOPE,
                "Items",
                TablePageRequest::new(1, 2).unwrap(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(result.rows(), expected(1, 2));
            assert!(!result.reached_end());
            assert_eq!(input.count(map_fixture::FIRST_DATA_PAGE as u64), 1);
            assert_eq!(input.count((map_fixture::FIRST_DATA_PAGE + 2) as u64), 1);
            assert_eq!(input.count((map_fixture::FIRST_DATA_PAGE + 3) as u64), 0);
            assert_eq!(
                read_table(
                    &mut Cursor::new(bytes),
                    SCOPE,
                    "Items",
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
fn later_map_corruption_still_prevents_first_page_and_physical_read_limits_remain() {
    for indirect in [false, true] {
        let bytes = map_fixture::database(6000, indirect, true);
        let mut input = PageReads::new(bytes);
        assert_eq!(
            read_table_page(
                &mut input,
                SCOPE,
                "Items",
                TablePageRequest::new(0, 1).unwrap(),
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::Malformed)
        );
        assert_eq!(input.count(map_fixture::FIRST_DATA_PAGE as u64), 0);
    }
    let bytes = map_fixture::database(6000, true, false);
    assert_eq!(
        read_table_page(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            TablePageRequest::new(5000, 1).unwrap(),
            &mut ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000),
            &|| false
        )
        .err(),
        Some(InspectError::StructureLimit)
    );
}
