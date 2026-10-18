use super::*;
use support::put16;

#[test]
fn native_binary_read_limits_preserve_legacy_system_metadata() {
    // Independent candidate layout: NBR_VALUE is the second column block at 124.
    for (kind, length, expected) in [
        (9, 0, None),
        (9, 1, Some(ScalarType::Binary)),
        (9, 255, Some(ScalarType::Binary)),
        (9, 256, Some(ScalarType::Binary)),
        (9, 510, Some(ScalarType::Binary)),
        (9, 511, None),
        (10, 2, Some(ScalarType::Text)),
        (10, 510, Some(ScalarType::Text)),
        (10, 509, None),
        (10, 512, None),
    ] {
        let mut source = copy();
        source.get_mut()[726 * 4096 + 124] = kind;
        put16(source.get_mut(), 726 * 4096 + 124 + 23, length);
        let schema = inspect_table_schema(
            &mut source,
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            schema.columns()[1].scalar_type(),
            expected,
            "kind={kind} length={length}"
        );
        assert_eq!(schema.scalar_columns().is_ok(), expected.is_some());
    }
    for table in ["MSysObjects", "MSysACEs"] {
        let schema = inspect_table_schema(
            &mut copy(),
            CatalogScope::new(1),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let wide: Vec<_> = schema
            .columns()
            .iter()
            .filter(|c| c.native_type() == 9 && c.storage_length() == 510)
            .collect();
        assert!(!wide.is_empty());
        assert!(
            wide.iter()
                .all(|c| c.scalar_type() == Some(ScalarType::Binary))
        );
        assert!(schema.scalar_columns().is_ok());
    }
}
