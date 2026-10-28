//! Owned request keys retain ROW semantics independently of their wire bytes.
use super::*;
use crate::{
    inspect_table_indexes,
    table::indexes::{
        IndexColumn, binary, encoded_tuple,
        key::{KeyProfile, KeyWork},
        mutation::{
            encoding,
            retained::{Preserved, RetainedProfile},
        },
        page::{Entry, KeyRef},
        profile::Coverage,
    },
    test_support::encoded::split_model::{self, Family},
};

fn source_checked(bytes: &[u8]) -> TableCheck {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|entry| entry.status() == IndexCheckStatus::Checked)
    );
    checked
}

#[test]
fn original_text_digest_and_binary_segments_survive_typed_preparation_and_erasure() {
    for family in [Family::Modern, Family::Binary] {
        for descending in [false, true] {
            let mut source = split_model::small_source(family, descending);
            let binary = matches!(family, Family::Binary);
            if binary {
                support::put16(&mut source.bytes, 10 * PAGE + 123, 510);
            }
            let checked = source_checked(&source.bytes);
            let image = crate::test_support::encoded::model::capture(&source.bytes);
            let value = if binary {
                Value::Binary(vec![0xa5; 510])
            } else {
                Value::Text("a".repeat(255))
            };
            let values = [Value::Null, value];
            // NULL outside the selected physical key has no constraint meaning.
            let request = [InsertRow {
                reference: (12 << 8) | 9,
                values: &values,
            }];
            let prepared =
                prepare(&image, &checked, 0, &request, &mut budget(), &|| false).unwrap();
            assert_eq!(prepared.source_pages(), &[13]);
            let (plan, append) = prepared
                .continue_with_append(
                    &image,
                    &checked,
                    State::new(&image, 10),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(append.length, source.bytes.len());
            assert!(plan.added.is_empty());
            assert_eq!(plan.original_count(), 9);
            for (ordinal, original) in source.keys.iter().enumerate() {
                let row = (12 << 8) | ordinal as u32;
                assert!(
                    plan.expected()
                        .any(|(wire, reference)| wire == original && reference == row)
                );
            }
            let wire = plan
                .expected()
                .find(|(_, row)| *row == ((12 << 8) | 9))
                .unwrap()
                .0;
            if binary {
                assert_eq!(wire, split_model::binary_key(&vec![0xa5; 510], descending));
                assert_eq!(wire.len(), 577);
            } else {
                assert_eq!(wire, source.keys[8]);
                assert_eq!(wire.len(), 510);
            }
        }
    }
}

#[test]
fn digested_tuple_null_facts_survive_retention_but_separator_decoding_stays_factless() {
    let source = split_model::small_source(Family::Modern, false);
    let mut inspected = inspect_table_indexes(
        &mut Cursor::new(&source.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut index = inspected.indexes.remove(0);
    index.columns.clear();
    index.columns.push(IndexColumn {
        number: 1,
        descending: false,
    });
    index.columns.push(IndexColumn {
        number: 0,
        descending: true,
    });
    let profile = Preserved(
        encoded_tuple::EncodedTupleProfile::new(
            &index,
            inspected.schema(),
            Coverage::CompleteTable,
        )
        .unwrap(),
    );
    let mut allowance = budget();
    let mut work = KeyWork {
        budget: &mut allowance,
        cancelled: &|| false,
    };
    let row = [Value::Null, Value::Text("a".repeat(255))];
    let key = profile.row_key_with_wire(&row, &mut work).unwrap();
    assert_eq!(profile.constraint_nulls(&key).unwrap(), (false, true));
    assert!(!profile.unique_key_is_exact(&key));
    let retained = profile.retain(&key, &mut work).unwrap();
    assert_eq!(profile.constraint_nulls(&retained).unwrap(), (false, true));
    let entry = Entry {
        value: KeyRef {
            key: &retained,
            row: 0x0c09,
        },
        child: 123,
    };
    let encoded =
        encoding::borrowed_entry::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(&entry, &profile)
            .unwrap();
    assert_eq!(encoded.length, 518);
    let wire = &encoded.bytes[..510];
    let (separator, end) = profile.decode(wire, &mut work).unwrap();
    assert_eq!(end, 510);
    assert_eq!(
        profile.constraint_nulls(&separator),
        Err(InspectError::Malformed)
    );
    assert!(profile.compare(&retained, &separator).is_eq());
    let bound = profile.bind_row(&separator, &row, &mut work).unwrap();
    assert_eq!(profile.constraint_nulls(&bound).unwrap(), (false, true));
    let mut altered = wire.to_vec();
    altered[509] ^= 1;
    let (altered, _) = profile.decode(&altered, &mut work).unwrap();
    assert!(profile.bind_row(&altered, &row, &mut work).is_err());
    let all_null = profile
        .row_key_with_wire(&[Value::Null, Value::Null], &mut work)
        .unwrap();
    assert_eq!(profile.constraint_nulls(&all_null).unwrap(), (true, true));
}

#[test]
fn wide_owned_keys_pay_for_retention_and_observe_cancellation() {
    let profile = Preserved(binary::BinaryProfile {
        column: 0,
        descending: true,
        maximum: 510,
        selection: binary::BinaryIndexSelection::All,
    });
    let values = [Value::Binary(vec![0x81; 510])];
    let mut allowance = budget();
    let key = profile
        .row_key_with_wire(
            &values,
            &mut KeyWork {
                budget: &mut allowance,
                cancelled: &|| false,
            },
        )
        .unwrap();
    let encoded = encoding::borrowed_entry::<_, { binary::MAX_ENTRY_BYTES }>(
        &Entry {
            value: KeyRef {
                key: &key,
                row: 0x0c09,
            },
            child: 123,
        },
        &profile,
    )
    .unwrap();
    assert_eq!(encoded.length, 585);
    let expected = split_model::binary_key(&vec![0x81; 510], true);
    assert_eq!(&encoded.bytes[..577], expected);
    let mut exhausted = ResourceBudget::new(0, 100_000, 100_000_000);
    assert!(
        profile
            .retain(
                &key,
                &mut KeyWork {
                    budget: &mut exhausted,
                    cancelled: &|| false
                }
            )
            .is_err()
    );
    assert!(matches!(
        profile.retain(
            &key,
            &mut KeyWork {
                budget: &mut allowance,
                cancelled: &|| true
            }
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        profile.row_key_with_wire(
            &values,
            &mut KeyWork {
                budget: &mut allowance,
                cancelled: &|| true
            }
        ),
        Err(InspectError::Cancelled)
    ));
}

#[test]
fn source_hydration_rejects_record_drift_before_any_index_allocation() {
    let source = split_model::small_source(Family::Modern, false);
    let checked = source_checked(&source.bytes);
    let mut changed = source.bytes.clone();
    let start =
        u16::from_le_bytes(changed[12 * PAGE + 18..12 * PAGE + 20].try_into().unwrap()) as usize;
    assert_eq!(
        &changed[12 * PAGE + start + 6..12 * PAGE + start + 8],
        &[b'a', 0]
    );
    changed[12 * PAGE + start + 6] = b'z';
    let image = crate::test_support::encoded::model::capture(&changed);
    assert!(prepare(&image, &checked, 0, &[], &mut budget(), &|| false).is_err());
    let image = crate::test_support::encoded::model::capture(&source.bytes);
    let prepared = prepare(&image, &checked, 0, &[], &mut budget(), &|| false).unwrap();
    let (plan, append) = prepared
        .continue_with_append(
            &image,
            &checked,
            State::new(&image, 10),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(plan.patches.is_empty());
    assert!(plan.added.is_empty());
    assert_eq!(append.length, source.bytes.len());
}
