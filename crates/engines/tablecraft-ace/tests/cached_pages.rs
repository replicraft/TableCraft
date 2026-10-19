#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "calculated_storage/scalar_fixture.rs"]
mod scalar_fixture;
mod support;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    InspectError, StoredTablePage, TablePageRequest, read_stored_table_page, read_table_page,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const FIRST_FIELD_OFFSET: usize = 63;
const FIELD_BYTES: usize = 25;
const EXTENDED_FLAGS_OFFSET: usize = 16;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8], offset: u32, limit: u32) -> Result<StoredTablePage, InspectError> {
    read_stored_table_page(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(offset, limit).unwrap(),
        &mut budget(),
        &|| false,
    )
}

#[test]
fn cached_pages_mix_exact_stored_carriers_and_keep_column_origins() {
    for code in [5, 6] {
        for (kind, capacity, compressed, payload, expected) in [
            (1, 39, false, vec![0xff], Value::Boolean(true)),
            (
                4,
                39,
                false,
                (-40_i32).to_le_bytes().to_vec(),
                Value::Int32(-40),
            ),
            (
                7,
                39,
                false,
                (-0.0_f64).to_bits().to_le_bytes().to_vec(),
                Value::Double(Double::from_bits((-0.0_f64).to_bits())),
            ),
            (
                5,
                39,
                false,
                i64::MIN.to_le_bytes().to_vec(),
                Value::Currency(Currency::from_scaled(i64::MIN)),
            ),
            (
                8,
                39,
                false,
                (-1.25_f64).to_bits().to_le_bytes().to_vec(),
                Value::DateTime(LegacyDateTime::from_bits((-1.25_f64).to_bits()).unwrap()),
            ),
            (
                10,
                103,
                false,
                "é🦀".encode_utf16().flat_map(u16::to_le_bytes).collect(),
                Value::Text("é🦀".into()),
            ),
            (
                10,
                103,
                true,
                b"\xff\xfeASCII".to_vec(),
                Value::Text("ASCII".into()),
            ),
        ] {
            let bytes =
                scalar_fixture::database(code, kind, capacity, compressed, &[None, Some(payload)]);
            let page = read(&bytes, 0, 3).unwrap();
            assert_eq!(
                page.rows(),
                &[
                    vec![Value::Int32(1), Value::Null],
                    vec![Value::Int32(2), expected.clone()]
                ]
            );
            assert_eq!(
                page.columns().iter().map(|c| c.name()).collect::<Vec<_>>(),
                ["Id", "Score"]
            );
            assert_eq!(page.is_stored_calculation(0), Some(false));
            assert_eq!(page.is_stored_calculation(1), Some(true));
            assert_eq!(page.is_stored_calculation(2), None);
            assert!(
                page.features()
                    .columns()
                    .nth(1)
                    .unwrap()
                    .expression()
                    .unwrap()
                    .is_some()
            );
            assert_eq!(page.request(), TablePageRequest::new(0, 3).unwrap());
            assert!(page.reached_end());
            assert!(matches!(
                read_table_page(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    TablePageRequest::new(0, 3).unwrap(),
                    &mut budget(),
                    &|| false
                ),
                Err(InspectError::Unsupported)
            ));
            let selected = read(&bytes, 1, 1).unwrap();
            assert_eq!(selected.rows(), &[vec![Value::Int32(2), expected]]);
            assert!(!selected.reached_end());
            assert!(read(&bytes, u32::MAX, 1).unwrap().rows().is_empty());
        }
    }
}

#[test]
fn all_declarations_are_required_even_when_empty_or_outside_the_requested_range() {
    let original = calculated_fixture::database(&[]);
    assert!(read(&original, 0, 1).unwrap().rows().is_empty());
    for property in ["Expression", "ResultType"] {
        let mut bytes = original.clone();
        let marker = property
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let at = bytes
            .windows(marker.len())
            .position(|part| part == marker)
            .unwrap();
        bytes[at] = b'Z'; // Absent required declaration, without corrupting geometry.
        assert!(matches!(
            read(&bytes, u32::MAX, 1),
            Err(InspectError::Unsupported)
        ));
    }
    let mut bytes = original.clone();
    bytes[DEFINITION_PAGE * PAGE_BYTES + FIRST_FIELD_OFFSET] = u8::MAX;
    assert!(matches!(read(&bytes, 0, 1), Err(InspectError::Unsupported)));
    let mut bytes = original;
    bytes[DEFINITION_PAGE * PAGE_BYTES
        + FIRST_FIELD_OFFSET
        + FIELD_BYTES
        + EXTENDED_FLAGS_OFFSET] |= 4;
    assert!(matches!(read(&bytes, 0, 1), Err(InspectError::Unsupported)));
}

#[test]
fn selected_corruption_is_atomic_but_skipped_and_later_payloads_remain_unchecked() {
    let bytes = scalar_fixture::database(6, 1, 39, false, &[Some(vec![0xff]), Some(vec![1])]);
    assert_eq!(
        read(&bytes, 0, 1).unwrap().rows(),
        &[vec![Value::Int32(1), Value::Boolean(true)]]
    );
    assert!(matches!(read(&bytes, 0, 2), Err(InspectError::Unsupported)));
    assert!(matches!(read(&bytes, 1, 1), Err(InspectError::Unsupported)));
    assert!(read(&bytes, 2, 1).unwrap().reached_end());
}

#[test]
fn cached_opt_in_does_not_qualify_calculations_for_older_profiles() {
    let mut bytes = calculated_fixture::database(&[]);
    for code in [2, 3] {
        bytes[20] = code;
        assert!(matches!(read(&bytes, 0, 1), Err(InspectError::Unsupported)));
    }
}

#[test]
fn cached_pages_retain_budget_and_final_cancellation_checks() {
    let bytes = calculated_fixture::database(&[Some(5)]);
    assert!(matches!(
        read_stored_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    read_stored_table_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(0, 1).unwrap(),
        &mut budget(),
        &count,
    )
    .unwrap();
    let final_call = calls.get();
    calls.set(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() == final_call
    };
    assert!(matches!(
        read_stored_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &cancel
        ),
        Err(InspectError::Cancelled)
    ));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:128, rng_seed:proptest::test_runner::RngSeed::Fixed(0x4341434845504147), ..Default::default()})]
    #[test]
    fn generated_pages_preserve_live_ordinals_and_nullable_cached_int32s(values in proptest::collection::vec(proptest::option::of(any::<i32>()), 0..16), offset in 0u32..20, limit in 1u32..8) {
        let bytes = calculated_fixture::database(&values);
        let page = read(&bytes, offset, limit).unwrap();
        let expected = values.iter().enumerate().skip(offset as usize).take(limit as usize).map(|(i, value)| vec![Value::Int32(i as i32 + 1), value.map(Value::Int32).unwrap_or(Value::Null)]).collect::<Vec<_>>();
        prop_assert_eq!(page.rows(), expected.as_slice());
    }
}
