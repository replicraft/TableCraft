#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "calculated_storage/extended_carriers.rs"]
mod extended_carriers;
use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "calculated_storage/scalar_fixture.rs"]
mod scalar_fixture;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, StoredCalculation, read_stored_calculation, read_stored_int32_calculation,
    read_table,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8]) -> Result<StoredCalculation, InspectError> {
    read_stored_calculation(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn stored_scalar_carriers_preserve_exact_bits_null_empty_and_boolean_mask_distinctions() {
    let cases = [
        (
            1,
            39,
            vec![Some(vec![0xff]), Some(vec![0]), None, Some(vec![])],
            vec![
                Value::Boolean(true),
                Value::Boolean(false),
                Value::Null,
                Value::Boolean(false),
            ],
            ScalarType::Boolean,
        ),
        (
            7,
            39,
            vec![
                Some((-0.0_f64).to_bits().to_le_bytes().to_vec()),
                Some(1_u64.to_le_bytes().to_vec()),
                None,
                Some(vec![]),
            ],
            vec![
                Value::Double(Double::from_bits((-0.0_f64).to_bits())),
                Value::Double(Double::from_bits(1)),
                Value::Null,
                Value::Null,
            ],
            ScalarType::Double,
        ),
        (
            5,
            39,
            vec![
                Some(i64::MIN.to_le_bytes().to_vec()),
                Some(i64::MAX.to_le_bytes().to_vec()),
                None,
                Some(vec![]),
            ],
            vec![
                Value::Currency(Currency::from_scaled(i64::MIN)),
                Value::Currency(Currency::from_scaled(i64::MAX)),
                Value::Null,
                Value::Null,
            ],
            ScalarType::Currency,
        ),
        (
            8,
            39,
            vec![
                Some((-1.25_f64).to_bits().to_le_bytes().to_vec()),
                Some(0_u64.to_le_bytes().to_vec()),
                None,
                Some(vec![]),
            ],
            vec![
                Value::DateTime(LegacyDateTime::from_bits((-1.25_f64).to_bits()).unwrap()),
                Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
                Value::Null,
                Value::Null,
            ],
            ScalarType::DateTime,
        ),
        (
            10,
            103,
            vec![
                Some(utf16("é🦀\t\n")),
                Some(vec![]),
                None,
                Some(utf16(&"x".repeat(40))),
            ],
            vec![
                Value::Text("é🦀\t\n".into()),
                Value::Text(String::new()),
                Value::Null,
                Value::Text("x".repeat(40)),
            ],
            ScalarType::Text,
        ),
    ];
    for code in [5, 6] {
        for (kind, capacity, payloads, expected, carrier) in &cases {
            let bytes = scalar_fixture::database(code, *kind, *capacity, false, payloads);
            let stored = read(&bytes).unwrap();
            assert_eq!(stored.values(), expected);
            assert_eq!(stored.data_type(), *carrier);
            assert_eq!(stored.column().storage_length(), usize::from(*capacity));
            assert!(stored.column().scalar_type().is_none());
            assert_eq!(
                read_table(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
            assert_eq!(
                read_stored_int32_calculation(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Score",
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
        }
    }
    let bytes = scalar_fixture::database(
        6,
        10,
        103,
        true,
        &[Some(b"\xff\xfeASCII".to_vec()), Some(vec![]), None],
    );
    assert_eq!(
        read(&bytes).unwrap().values(),
        &[
            Value::Text("ASCII".into()),
            Value::Text(String::new()),
            Value::Null
        ]
    );
    // A literal leading U+FEFF cannot be emitted as raw uncompressed UTF-16:
    // FF FE is the native compression marker. Enter wide mode explicitly.
    let escaped =
        scalar_fixture::database(6, 10, 103, true, &[Some(vec![0xff, 0xfe, 0, 0xff, 0xfe])]);
    assert_eq!(
        read(&escaped).unwrap().values(),
        &[Value::Text("\u{feff}".into())]
    );
}

#[test]
fn malformed_or_unknown_scalar_wrappers_return_no_partial_view_and_empty_tables_still_gate() {
    for (kind, capacity, payload, expected) in [
        (1, 39, vec![1], InspectError::Unsupported),
        (1, 39, vec![0, 0], InspectError::Malformed),
        (7, 39, vec![0; 7], InspectError::Malformed),
        (5, 39, vec![0; 9], InspectError::Malformed),
        (
            8,
            39,
            f64::NAN.to_bits().to_le_bytes().to_vec(),
            InspectError::Malformed,
        ),
        (10, 103, vec![0], InspectError::Malformed),
        (10, 103, vec![0, 0xd8], InspectError::Malformed),
        (10, 103, utf16(&"x".repeat(41)), InspectError::Malformed),
    ] {
        assert_eq!(
            read(&scalar_fixture::database(
                6,
                kind,
                capacity,
                false,
                &[None, Some(payload)]
            ))
            .err(),
            Some(expected)
        );
    }
    for (kind, capacity) in [
        (1, 0),
        (1, 38),
        (7, 38),
        (10, 22),
        (10, 24),
        (10, 535),
        (0x14, 39),
    ] {
        assert_eq!(
            read(&scalar_fixture::database(6, kind, capacity, false, &[])).err(),
            Some(InspectError::Unsupported)
        );
    }
    for kind in [1, 7, 5, 8, 10] {
        let bytes =
            scalar_fixture::database(6, kind, if kind == 10 { 103 } else { 39 }, false, &[]);
        assert!(read(&bytes).unwrap().values().is_empty());
        let mut unknown = bytes.clone();
        unknown[10 * 4096 + 63 + 25 + 16] |= 0x04;
        assert_eq!(read(&unknown).err(), Some(InspectError::Unsupported));
        let mut old = bytes;
        old[20] = 2;
        assert_eq!(read(&old).err(), Some(InspectError::Unsupported));
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x43414c435343414c),..Default::default()})]
    #[test]
    fn exact_scalars_match_independent_stored_bits_and_utf16_units(bits in any::<u64>(),scaled in any::<i64>(),chars in proptest::collection::vec(any::<char>(),0..20)) {
        let doubles=scalar_fixture::database(6,7,39,false,&[Some(bits.to_le_bytes().to_vec())]);
        let stored=read(&doubles).unwrap();
        prop_assert_eq!(stored.values(),&[Value::Double(Double::from_bits(bits))]);
        let currency=scalar_fixture::database(6,5,39,false,&[Some(scaled.to_le_bytes().to_vec())]);
        let stored=read(&currency).unwrap();
        prop_assert_eq!(stored.values(),&[Value::Currency(Currency::from_scaled(scaled))]);
        // Emit valid uncompressed native text: an ordinary leading unit keeps
        // arbitrary Unicode from impersonating the reserved compression marker.
        let text=std::iter::once('A').chain(chars).collect::<String>();
        let bytes=scalar_fixture::database(6,10,103,false,&[Some(utf16(&text))]);
        let stored=read(&bytes).unwrap();prop_assert_eq!(stored.values(),&[Value::Text(text)]);
    }
}

#[test]
fn generic_stored_reads_keep_late_cancellation_and_exhausted_budgets() {
    use std::cell::Cell;
    let bytes = scalar_fixture::database(6, 10, 103, false, &[Some(utf16("value"))]);
    let calls = Cell::new(0_usize);
    read_stored_calculation(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let at = Cell::new(0);
    assert_eq!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| {
                at.set(at.get() + 1);
                at.get() >= calls.get() - 1
            }
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(matches!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut ResourceBudget::new(66, 1, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
