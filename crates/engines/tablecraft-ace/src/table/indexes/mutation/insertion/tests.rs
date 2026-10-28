use super::*;
use crate::{
    IndexConstraintViolation, check_table, check_table_constraints,
    test_support::{index_fixture, split_model},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{LimitError, Resource};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const ROOT_START: usize = 13 * 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8, alias: bool) -> Vec<u8> {
    let mut bytes = index_fixture::database(false, alias, false);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes[10 * PAGE + 189] = 0x89;
    bytes[10 * PAGE + 218] = 1;
    bytes
}

fn checked(bytes: &[u8]) -> TableCheck {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), report.rows());
    report
}

fn literal_leaf(original: &[u8], keys: &[(i32, u32)]) -> Vec<u8> {
    let entries = keys
        .iter()
        .map(|(value, row)| {
            [
                vec![127],
                ((*value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
                row.to_be_bytes().to_vec(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    let mut common = entries.first().map_or(0, Vec::len);
    for entry in &entries[1..] {
        common = entries[0][..common]
            .iter()
            .zip(entry)
            .take_while(|(left, right)| left == right)
            .count();
    }
    let seed = usize::from(u16::from_le_bytes(original[24..26].try_into().unwrap()));
    let saved = entries.len().saturating_sub(1);
    let mut prefix = seed.min(common);
    if entries.len() * 9 - prefix * saved > 3616 {
        prefix = common;
    }
    let active = entries.len() * 9 - prefix * saved;
    assert!(active <= 3616);
    let mut bytes = original.to_vec();
    bytes[27..480].fill(0);
    support::put16(&mut bytes, 2, (3616 - active) as u16);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        bytes[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    assert_eq!(&bytes[480 + active..], &original[480 + active..]);
    bytes
}

fn compose_root(before: &[u8], plan: &PrimaryInsertion) -> Vec<u8> {
    let mut bytes = before.to_vec();
    for patch in &plan.patches {
        assert_eq!(
            &bytes[patch.offset..patch.offset + patch.before.len()],
            patch.before
        );
        assert!(
            patch.offset >= ROOT_START && patch.offset + patch.after.len() <= ROOT_START + PAGE
        );
        bytes[patch.offset..patch.offset + patch.after.len()].copy_from_slice(&patch.after);
    }
    bytes
}

#[test]
fn genuine_primary_insertion_grows_cardinality_once_for_logical_aliases() {
    for code in [5, 6] {
        for alias in [false, true] {
            let before = source(code, alias);
            let check = checked(&before);
            let value = Value::Int32(25);
            let request = [InsertKey {
                reference: (12 << 8) | 3,
                value: &value,
            }];
            let expected_entries = [
                (10, 12 << 8),
                (20, (12 << 8) | 1),
                (25, (12 << 8) | 3),
                (30, (12 << 8) | 2),
            ];
            let expected_root =
                literal_leaf(&before[ROOT_START..ROOT_START + PAGE], &expected_entries);
            let image = split_model::capture(&before);
            let plan = prepare(&image, &check, 1, &request, &mut budget(), &|| false).unwrap();
            assert_eq!(
                (
                    plan.physical(),
                    plan.root(),
                    plan.usage(),
                    plan.original_count()
                ),
                (0, 13, 14 << 8, 3)
            );
            assert_eq!(
                plan.expected().collect::<Vec<_>>(),
                expected_entries
                    .iter()
                    .map(|(key, row)| (Some(i64::from(*key)), *row))
                    .collect::<Vec<_>>()
            );
            let mut after = compose_root(&before, &plan);
            assert_eq!(&after[ROOT_START..ROOT_START + PAGE], expected_root);
            assert_eq!(&after[..ROOT_START], &before[..ROOT_START]);
            assert_eq!(&after[ROOT_START + PAGE..], &before[ROOT_START + PAGE..]);

            // Supply the separate DATA/counter phase literally, rather than
            // asking the index helper to change a record or historical counter.
            let row = [2, 0, 4, 0, 0, 0, 25, 0, 0, 0, 3];
            let end = usize::from(u16::from_le_bytes(
                before[12 * PAGE + 18..12 * PAGE + 20].try_into().unwrap(),
            ));
            let start = end - row.len();
            after[12 * PAGE + start..12 * PAGE + end].copy_from_slice(&row);
            support::put16(&mut after, 12 * PAGE + 20, start as u16);
            support::put16(&mut after, 12 * PAGE + 12, 4);
            support::put16(&mut after, 12 * PAGE + 2, (start - 22) as u16);
            support::put32(&mut after, 10 * PAGE + 16, 4);
            support::put32(&mut after, 10 * PAGE + 67, 4);
            assert_eq!(checked(&after).rows(), 4);
        }
    }
}

#[test]
fn unordered_new_keys_keep_exact_signed_domain_and_complete_row_ties() {
    let before = source(6, true);
    let check = checked(&before);
    let values = [
        Value::Int32(i32::MAX),
        Value::Int32(i32::MIN),
        Value::Int32(-1),
    ];
    let requests = values
        .iter()
        .enumerate()
        .map(|(slot, value)| InsertKey {
            reference: (15 << 8) | slot as u32,
            value,
        })
        .collect::<Vec<_>>();
    let expected_entries = [
        (i32::MIN, (15 << 8) | 1),
        (-1, (15 << 8) | 2),
        (10, 12 << 8),
        (20, (12 << 8) | 1),
        (30, (12 << 8) | 2),
        (i32::MAX, 15 << 8),
    ];
    let expected = literal_leaf(&before[ROOT_START..ROOT_START + PAGE], &expected_entries);
    let image = split_model::capture(&before);
    let plan = prepare(&image, &check, 1, &requests, &mut budget(), &|| false).unwrap();
    assert_eq!(plan.expected().len(), 6);
    assert_eq!(
        plan.expected().collect::<Vec<_>>(),
        expected_entries
            .iter()
            .map(|(key, row)| (Some(i64::from(*key)), *row))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &compose_root(&before, &plan)[ROOT_START..ROOT_START + PAGE],
        expected
    );
}

#[test]
fn null_wrong_carriers_duplicate_keys_and_reused_logical_rows_are_refused() {
    let before = source(5, true);
    let check = checked(&before);
    let image = split_model::capture(&before);
    for value in [Value::Null, Value::Int64(40), Value::Binary(vec![40])] {
        let requests = [InsertKey {
            reference: (12 << 8) | 3,
            value: &value,
        }];
        assert!(matches!(
            prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
    }
    let duplicate = Value::Int32(20);
    let requests = [InsertKey {
        reference: (12 << 8) | 3,
        value: &duplicate,
    }];
    assert!(matches!(
        prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    ));
    let value = Value::Int32(40);
    for reference in [12 << 8, 0, (12 << 8) | 255] {
        let requests = [InsertKey {
            reference,
            value: &value,
        }];
        assert!(matches!(
            prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
    }
    let requests = [
        InsertKey {
            reference: (12 << 8) | 3,
            value: &value,
        },
        InsertKey {
            reference: (12 << 8) | 3,
            value: &duplicate,
        },
    ];
    assert!(matches!(
        prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
        Err(EditError::Conflict)
    ));
    let requests = [
        InsertKey {
            reference: (12 << 8) | 3,
            value: &value,
        },
        InsertKey {
            reference: (12 << 8) | 4,
            value: &value,
        },
    ];
    assert!(matches!(
        prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        )))
    ));
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn all_index_profiles_and_existing_write_headers_are_qualified_without_fallback() {
    let value = Value::Int32(40);
    for mode in 0..4 {
        let mut before = source(6, false);
        match mode {
            0 => before[10 * PAGE + 218] = 0,
            1 => before[10 * PAGE + 189] |= 2,
            2 => before[ROOT_START + 8] = 0x42,
            _ => {}
        }
        let check = checked(&before);
        let image = split_model::capture(&before);
        let request = [InsertKey {
            reference: (12 << 8) | 3,
            value: &value,
        }];
        let column = if mode == 3 { 0 } else { 1 };
        assert!(matches!(
            prepare(&image, &check, column, &request, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
    }
}

#[test]
fn overflow_prepares_no_leaf_patch_before_explicit_split_continuation() {
    let mut before = split_model::source(6, false, true);
    before[10 * PAGE + 189] = 0x89;
    before[10 * PAGE + 218] = 1;
    let check = checked(&before);
    assert_eq!(check.rows(), 500);
    let image = split_model::capture(&before);
    let value = Value::Int32(i32::MAX);
    let requests = [InsertKey {
        reference: (15 << 8) | 250,
        value: &value,
    }];
    let pending = prepare(&image, &check, 1, &requests, &mut budget(), &|| false).unwrap();
    assert!(pending.requires_continuation);
    assert_eq!(pending.expected().len(), 501);
    assert!(pending.patches.is_empty());
    assert!(pending.added.is_empty());
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn pools_late_cancellation_retry_and_empty_requests_preserve_source_bytes() {
    let before = source(6, true);
    let check = checked(&before);
    let image = split_model::capture(&before);
    let value = Value::Int32(40);
    let requests = [InsertKey {
        reference: (12 << 8) | 3,
        value: &value,
    }];
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut allowance = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
            _ => unreachable!(),
        };
        assert!(
            matches!(prepare(&image, &check, 1, &requests, &mut allowance, &|| false), Err(EditError::Inspect(InspectError::Limit(LimitError { resource: actual, .. }))) if actual == resource)
        );
    }
    let calls = Cell::new(0);
    let cancelled = || {
        let prior = calls.get();
        calls.set(prior + 1);
        prior >= 10
    };
    assert!(matches!(
        prepare(&image, &check, 1, &requests, &mut budget(), &cancelled),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let healthy = prepare(&image, &check, 1, &requests, &mut budget(), &|| false).unwrap();
    assert_eq!(healthy.expected().len(), 4);
    let noop = prepare(&image, &check, 1, &[], &mut budget(), &|| false).unwrap();
    assert!(noop.patches.is_empty());
    assert_eq!(noop.expected().len(), 3);
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn another_genuine_physical_index_is_not_silently_skipped() {
    let mut before = source(6, false);
    before.resize(18 * PAGE, 0);
    let original = before[10 * PAGE..11 * PAGE].to_vec();
    let definition = &mut before[10 * PAGE..11 * PAGE];
    definition[63..].fill(0);
    support::put32(definition, 47, 2);
    support::put32(definition, 51, 2);
    support::put32(definition, 67, 3);
    support::put32(definition, 79, 3);
    definition[87..155].copy_from_slice(&original[75..143]);
    definition[155..207].copy_from_slice(&original[143..195]);
    definition[207..259].copy_from_slice(&original[143..195]);
    support::put32(definition, 207 + 34, 17 << 8);
    support::put32(definition, 207 + 38, 16);
    definition[259..287].copy_from_slice(&original[195..223]);
    definition[287..315].copy_from_slice(&original[195..223]);
    support::put32(definition, 287 + 4, 1);
    support::put32(definition, 287 + 8, 1);
    definition[287 + 23] = 0;
    let mut names = 315;
    for name in ["ScoreKey", "OtherKey"] {
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(definition, names, encoded.len() as u16);
        names += 2;
        definition[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    let root = before[ROOT_START..ROOT_START + PAGE].to_vec();
    before[16 * PAGE..17 * PAGE].copy_from_slice(&root);
    let mut owned = vec![0; 69];
    owned[7] = 1;
    before[17 * PAGE..18 * PAGE].copy_from_slice(&support::data_page(0, &[(0, owned)]));
    let check = checked(&before);
    assert_eq!(check.indexes().len(), 2);
    assert_eq!(check.indexes()[0].index().data_number(), 0);
    assert_eq!(check.indexes()[1].index().data_number(), 1);
    let image = split_model::capture(&before);
    let value = Value::Int32(40);
    let requests = [InsertKey {
        reference: (12 << 8) | 3,
        value: &value,
    }];
    assert!(matches!(
        prepare(&image, &check, 1, &requests, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
}

#[test]
fn empty_primary_leaf_accepts_its_first_key_without_reallocating_or_canonicalizing_noop() {
    let mut before = source(6, false);
    support::put32(&mut before, 10 * PAGE + 16, 0);
    support::put32(&mut before, 10 * PAGE + 67, 7);
    before[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(10, &[]));
    let root = &mut before[ROOT_START..ROOT_START + PAGE];
    root[27..480].fill(0);
    support::put16(root, 2, 3616);
    support::put16(root, 24, 0);
    let check = checked(&before);
    assert_eq!(check.rows(), 0);
    let image = split_model::capture(&before);
    let noop = prepare(&image, &check, 1, &[], &mut budget(), &|| false).unwrap();
    assert_eq!(noop.original_count(), 0);
    assert!(noop.patches.is_empty());
    let value = Value::Int32(i32::MIN);
    let requests = [InsertKey {
        reference: 12 << 8,
        value: &value,
    }];
    let expected = literal_leaf(
        &before[ROOT_START..ROOT_START + PAGE],
        &[(i32::MIN, 12 << 8)],
    );
    let plan = prepare(&image, &check, 1, &requests, &mut budget(), &|| false).unwrap();
    assert_eq!(
        plan.expected().collect::<Vec<_>>(),
        [(Some(i64::from(i32::MIN)), 12 << 8)]
    );
    assert_eq!(
        &compose_root(&before, &plan)[ROOT_START..ROOT_START + PAGE],
        expected
    );
    assert_eq!(
        &image.as_bytes()[10 * PAGE + 67..10 * PAGE + 71],
        &7_u32.to_le_bytes()
    );
}
