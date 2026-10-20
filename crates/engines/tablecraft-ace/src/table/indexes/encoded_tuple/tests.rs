use super::super::{numeric::NumericKey, profile::KeyType};
use super::*;
use crate::NativeColumn;
use crate::table::indexes::key::TestKeyProfile;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

fn schema() -> TableSchema {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/examples/Access_Example_VBA.accdb"
    ));
    let mut source = std::io::Cursor::new(bytes.as_slice());
    let mut schema = crate::inspect_table_schema(
        &mut source,
        tablecraft_model::CatalogScope::new(1),
        "TAG_GRP_TB",
        &mut ResourceBudget::new(16 * 1024 * 1024, 16384, 1_000_000),
        &|| false,
    )
    .unwrap();
    // Authored unit metadata over the registered schema; not native evidence.
    schema.columns[1].field.length = 510;
    schema.columns[1].field.sort_order =
        Some(crate::NativeSortOrder::from_native_bytes([9, 4, 0, 0]));
    schema
}
fn pair(
    field: &NativeColumn,
    text_descending: bool,
    number_descending: bool,
) -> EncodedTupleProfile<'_> {
    let mut columns = std::array::from_fn(|_| None);
    columns[0] = Some(Component {
        column: 0,
        descending: text_descending,
        kind: Kind::Text(field),
    });
    columns[1] = Some(Component {
        column: 1,
        descending: number_descending,
        kind: Kind::Numeric(KeyType::Int32, NumericKey::new(ScalarType::Int32).unwrap()),
    });
    EncodedTupleProfile {
        columns,
        count: 2,
        memo: false,
    }
}

#[test]
fn literal_mixed_direction_keys_and_row_null_facts_are_separate() {
    let schema = schema();
    let policy = pair(&schema.columns[1], false, true);
    // Independent component goldens: GeneralLegacy 'a', descending Long -1.
    let bytes = [0x7f, 0x4a, 1, 0, 0x80, 0x80, 0, 0, 0];
    let row = [Value::Text("A  ".into()), Value::Int32(-1)];
    let key = policy.row_key_test(&row).unwrap();
    assert_eq!(key.bytes, bytes);
    assert_eq!(policy.constraint_nulls(&key).unwrap(), (false, false));
    let (decoded, end) = policy.decode_test(&bytes).unwrap();
    assert!(decoded == key);
    assert_eq!(end, bytes.len());
    assert_eq!(
        policy.constraint_nulls(&decoded),
        Err(InspectError::Malformed)
    );
    for (row, bytes, nulls) in [
        ([Value::Null, Value::Null], vec![0, 255], (true, true)),
        (
            [Value::Text(String::new()), Value::Null],
            vec![127, 1, 0, 255],
            (false, true),
        ),
        (
            [Value::Null, Value::Int32(0)],
            vec![0, 128, 127, 255, 255, 255],
            (false, true),
        ),
    ] {
        let key = policy.row_key_test(&row).unwrap();
        assert_eq!(key.bytes, bytes);
        assert_eq!(policy.constraint_nulls(&key).unwrap(), nulls);
    }
}

#[test]
fn overflow_digest_covers_later_components_and_uniqueness_stays_unqualified() {
    let mut schema = schema();
    schema.columns[1].field.sort_order =
        Some(crate::NativeSortOrder::from_native_bytes([9, 4, 0, 1]));
    let policy = pair(&schema.columns[1], false, false);
    let first = policy
        .row_key_test(&[Value::Text("a".repeat(255)), Value::Int32(1)])
        .unwrap();
    let second = policy
        .row_key_test(&[Value::Text("a".repeat(255)), Value::Int32(2)])
        .unwrap();
    assert_eq!(first.bytes.len(), 510);
    assert_eq!(first.bytes[..508], second.bytes[..508]);
    assert_ne!(first.bytes[508..], second.bytes[508..]);
    assert!(!policy.unique_key_is_exact(&first));
    let short = policy
        .row_key_test(&[Value::Text("a".into()), Value::Int32(1)])
        .unwrap();
    assert!(policy.unique_key_is_exact(&short));
}

#[test]
fn tuple_carriers_missing_fields_allocations_and_cancellation_fail_explicitly() {
    let schema = schema();
    let policy = pair(&schema.columns[1], false, false);
    for row in [
        vec![Value::Int32(1), Value::Int32(2)],
        vec![Value::Text("a".into())],
        vec![Value::Null, Value::Text("bad".into())],
    ] {
        assert!(matches!(
            policy.row_key_test(&row),
            Err(InspectError::Malformed)
        ));
    }
    for bytes in [vec![], vec![0], vec![42], vec![127; 511]] {
        assert!(matches!(
            policy.decode_test(&bytes),
            Err(InspectError::Malformed)
        ));
    }
    let mut budget = ResourceBudget::new(0, 100, 1000);
    assert!(
        policy
            .decode(
                &[0, 0],
                &mut KeyWork {
                    budget: &mut budget,
                    cancelled: &|| false
                }
            )
            .is_err()
    );
    let mut budget = ResourceBudget::new(1024, 100, 1000);
    assert!(matches!(
        policy.row_key(
            &[Value::Null, Value::Null],
            &mut KeyWork {
                budget: &mut budget,
                cancelled: &|| true
            }
        ),
        Err(InspectError::Cancelled)
    ));
}

fn binary_pair(descending: bool) -> EncodedTupleProfile<'static> {
    let mut columns = std::array::from_fn(|_| None);
    for (position, column) in columns[..2].iter_mut().enumerate() {
        *column = Some(Component {
            column: position,
            descending,
            kind: Kind::Binary(255),
        });
    }
    EncodedTupleProfile {
        columns,
        count: 2,
        memo: false,
    }
}

#[test]
fn binary_tuple_goldens_preserve_empty_null_and_segment_boundaries() {
    for (descending, first, second, expected) in [
        (false, Value::Null, Value::Binary(vec![]), vec![0, 127]),
        (true, Value::Binary(vec![]), Value::Null, vec![128, 255]),
        (
            false,
            Value::Binary(vec![0; 8]),
            Value::Binary(vec![255]),
            vec![
                127, 0, 0, 0, 0, 0, 0, 0, 0, 8, 127, 255, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        ),
        (
            true,
            Value::Binary(vec![255; 9]),
            Value::Binary(vec![]),
            vec![
                128, 0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 255, 255, 255, 255, 255, 255, 255, 254, 128,
            ],
        ),
    ] {
        let profile = binary_pair(descending);
        let key = profile.row_key_test(&[first, second]).unwrap();
        assert_eq!(key.bytes, expected);
        assert!(key == profile.decode_test(&expected).unwrap().0);
    }
    let profile = binary_pair(false);
    for (row, nulls) in [
        ([Value::Null, Value::Null], (true, true)),
        ([Value::Null, Value::Binary(vec![])], (false, true)),
        (
            [Value::Binary(vec![]), Value::Binary(vec![])],
            (false, false),
        ),
    ] {
        let key = profile.row_key_test(&row).unwrap();
        assert_eq!(profile.constraint_nulls(&key).unwrap(), nulls);
    }
    for row in [
        [Value::Binary(vec![0; 256]), Value::Null],
        [Value::Text("wrong".into()), Value::Null],
    ] {
        assert!(matches!(
            profile.row_key_test(&row),
            Err(InspectError::Malformed)
        ));
    }
}

#[test]
fn binary_tuple_digest_covers_both_complete_components_and_refuses_unique_proof() {
    let profile = binary_pair(false);
    let first = profile
        .row_key_test(&[Value::Binary(vec![255; 255]), Value::Binary(vec![0; 255])])
        .unwrap();
    let mut changed = vec![0; 255];
    changed[254] = 1;
    let second = profile
        .row_key_test(&[Value::Binary(vec![255; 255]), Value::Binary(changed)])
        .unwrap();
    assert_eq!(first.bytes.len(), 510);
    assert_eq!(first.bytes[..508], second.bytes[..508]);
    assert_ne!(first.bytes[508..], second.bytes[508..]);
    assert!(!profile.unique_key_is_exact(&first));
}
