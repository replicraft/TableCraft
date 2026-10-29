//! Independent SOURCE and complete-image expectations for omitted UPDATE keys.
mod boundaries;
mod forwarded;
mod tuple;

use crate::table::mutation::insert::tests::multiple_indexes::model::{self, Case, PAGE};
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, InspectError, NativeImage,
    WriteBlocker, check_table, check_table_constraints, read_table,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(values: &[Option<i32>; 3]) -> Vec<FixedScalarUpdate> {
    [Some(10), Some(10), None]
        .into_iter()
        .zip(values)
        .enumerate()
        .map(|(row, (before, after))| {
            FixedScalarUpdate::new(
                row,
                before.map_or(Value::Null, Value::Int32),
                after.map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

struct Example {
    source: Case,
    expected: Vec<u8>,
    final_values: [Option<i32>; 3],
}

fn example(code: u8, descending: bool, final_values: [Option<i32>; 3]) -> Example {
    let mut source = model::ignored_null_source(code);
    source.descending = descending;
    source.before[10 * PAGE + 250] = u8::from(!descending);
    let original_values = source
        .rows
        .iter()
        .map(|(row, _)| row.group)
        .collect::<Vec<_>>();
    let source_leaf = leaf(&source, &original_values, None);
    source.before[14 * PAGE..15 * PAGE].copy_from_slice(&source_leaf);

    let mut expected = source.before.clone();
    for (position, ((_, reference), value)) in source.rows.iter().zip(final_values).enumerate() {
        let page = (*reference >> 8) as usize * PAGE;
        let slot = (*reference & 255) as usize;
        let directory = page + 14 + slot * 2;
        let start = usize::from(u16::from_le_bytes(
            expected[directory..directory + 2].try_into().unwrap(),
        )) & 0x1fff;
        let end = if slot == 0 {
            PAGE
        } else {
            usize::from(u16::from_le_bytes(
                expected[directory - 2..directory].try_into().unwrap(),
            )) & 0x1fff
        };
        if original_values[position] != value {
            // The established masked UPDATE contract retains ignored NULL
            // payload bytes; only a present replacement writes this span.
            if let Some(value) = value {
                expected[page + start + 6..page + start + 10].copy_from_slice(&value.to_le_bytes());
            }
            expected[page + end - 1] =
                (expected[page + end - 1] & !2) | (u8::from(value.is_some()) << 1);
        }
    }
    let expected_leaf = leaf(&source, &final_values, Some(&source_leaf));
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&expected_leaf);

    // Only original omitted rows can increment an UPDATE history counter.
    // These fixtures deliberately avoid a removed/reinserted family ambiguity.
    let mut introduced = Vec::new();
    for (old, new) in original_values.iter().zip(final_values) {
        if old.is_none()
            && let Some(value) = new
            && !original_values.contains(&Some(value))
            && !introduced.contains(&value)
        {
            introduced.push(value);
        }
    }
    let history = source.histories[1] + introduced.len() as u32;
    expected[10 * PAGE + 79..10 * PAGE + 83].copy_from_slice(&history.to_le_bytes());
    Example {
        source,
        expected,
        final_values,
    }
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(offset, (left, right))| (offset, *left, *right))
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "{} differing bytes; first: {:?}",
        differences.len(),
        &differences[..differences.len().min(8)]
    );
}

fn leaf(source: &Case, values: &[Option<i32>], original: Option<&[u8]>) -> Vec<u8> {
    let mut entries = source
        .rows
        .iter()
        .zip(values)
        .filter_map(|((_, reference), value)| {
            value.map(|value| {
                let mut key = vec![127];
                key.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
                if source.descending {
                    for byte in &mut key {
                        *byte = !*byte;
                    }
                }
                key.extend(reference.to_be_bytes());
                key
            })
        })
        .collect::<Vec<_>>();
    entries.sort();
    model::leaf(original, &entries, 0, 0, 0)
}

fn qualify(bytes: &[u8], groups: &[Option<i32>]) {
    let mut reader = Cursor::new(bytes);
    let image =
        NativeImage::capture(&mut reader, CatalogScope::new(1), &mut budget(), &|| false).unwrap();
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 4);
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
    }
    check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let table = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (row, value) in table.rows().iter().zip(groups) {
        assert_eq!(
            row[1],
            value.map_or(
                tablecraft_core::values::Value::Null,
                tablecraft_core::values::Value::Int32
            )
        );
    }
}

#[test]
fn qualify_ignored_update_sources_and_whole_images_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            for values in [
                [Some(10), Some(10), Some(20)],
                [Some(10), Some(10), Some(10)],
                [None, Some(10), Some(20)],
                [None, None, None],
            ] {
                let case = example(code, descending, values);
                qualify(&case.source.before, &[Some(10), Some(10), None]);
                qualify(&case.expected, &case.final_values);
                assert_eq!(case.expected.len(), case.source.before.len());
                assert_eq!(case.expected[..10 * PAGE], case.source.before[..10 * PAGE]);
                assert_eq!(case.expected[15 * PAGE..], case.source.before[15 * PAGE..]);
            }
        }
    }
}

#[test]
fn fixed_ignored_updates_match_whole_images_and_physical_alias_histories() {
    for code in [5, 6] {
        for descending in [false, true] {
            for values in [
                [Some(10), Some(10), Some(20)],
                [Some(10), Some(10), Some(10)],
                [None, Some(10), Some(20)],
                [None, None, None],
            ] {
                let case = example(code, descending, values);
                let updates = requests(&values);
                let result = capture(&case.source.before)
                    .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                assert_image(result.candidate_bytes(), &case.expected);
            }
        }
    }
}

#[test]
fn batch_ignored_updates_and_noops_retain_complete_original_associations() {
    for code in [5, 6] {
        let case = example(code, true, [None, Some(10), Some(20)]);
        let updates = requests(&case.final_values);
        let result = capture(&case.source.before)
            .preview_ace16_batch_updates(
                &[FixedColumnUpdates::new("Items", "Group", &updates)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_image(result.candidate_bytes(), &case.expected);
        for updates in [Vec::new(), requests(&[Some(10), Some(10), None])] {
            let result = capture(&case.source.before)
                .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false)
                .unwrap();
            assert_image(result.candidate_bytes(), &case.source.before);
        }
    }
}

#[test]
fn ignored_update_history_refuses_ambiguous_family_timings_and_overflow() {
    let case = example(6, false, [Some(10), Some(10), None]);
    for values in [[None, None, Some(10)], [Some(20), Some(10), Some(20)]] {
        assert_eq!(
            capture(&case.source.before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Group",
                    &requests(&values),
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    let mut source = case.source.before;
    source[10 * PAGE + 79..10 * PAGE + 83].copy_from_slice(&u32::MAX.to_le_bytes());
    qualify(&source, &[Some(10), Some(10), None]);
    assert_eq!(
        capture(&source)
            .preview_ace16_fixed_updates(
                "Items",
                "Group",
                &requests(&[Some(10), Some(10), Some(20)]),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Inspect(InspectError::StructureLimit))
    );
}

#[test]
fn ignored_update_conflicts_and_each_resource_pool_publish_no_candidate() {
    let case = example(6, false, [Some(10), Some(10), Some(20)]);
    let wrong = [FixedScalarUpdate::new(
        2,
        Value::Int32(10),
        Value::Int32(20),
    )];
    assert_eq!(
        capture(&case.source.before)
            .preview_ace16_fixed_updates("Items", "Group", &wrong, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Conflict)
    );
    let repeated = [
        FixedScalarUpdate::new(2, Value::Null, Value::Int32(20)),
        FixedScalarUpdate::new(2, Value::Null, Value::Int32(30)),
    ];
    assert_eq!(
        capture(&case.source.before)
            .preview_ace16_fixed_updates("Items", "Group", &repeated, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Conflict)
    );
    let updates = requests(&case.final_values);
    for (mut budget, resource) in [
        (
            ResourceBudget::new(1, 100_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 1, 100_000_000),
            Resource::Rows,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 100_000, 1),
            Resource::Steps,
        ),
    ] {
        let result = capture(&case.source.before).preview_ace16_fixed_updates(
            "Items",
            "Group",
            &updates,
            &mut budget,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(limit)))
            if limit.resource == resource)
        );
    }
    let calls = Cell::new(0_usize);
    capture(&case.source.before)
        .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let stop = calls.get() - 1;
    let late = Cell::new(0_usize);
    assert_eq!(
        capture(&case.source.before)
            .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| {
                late.set(late.get() + 1);
                late.get() >= stop
            },)
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}
