//! Independent complete images patch final payloads while retaining logical ROWs.
mod authority;
use super::{PAGE, assert_image, budget, capture, example, model, qualify};
use crate::table::mutation::insert::tests::ignored_index_inserts::model as forwarding;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError};
use std::cell::Cell;
use tablecraft_core::values::Value;

struct Case {
    before: Vec<u8>,
    expected: Vec<u8>,
    values: [Option<i32>; 3],
    final_page: usize,
}

fn span(bytes: &[u8], page: usize, slot: usize) -> std::ops::Range<usize> {
    let base = page * PAGE;
    let start = usize::from(u16::from_le_bytes(
        bytes[base + 14 + slot * 2..base + 16 + slot * 2]
            .try_into()
            .unwrap(),
    )) & 0x1fff;
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[base + 12 + slot * 2..base + 14 + slot * 2]
                .try_into()
                .unwrap(),
        )) & 0x1fff
    };
    base + start..base + end
}

fn case(
    code: u8,
    descending: bool,
    slot: usize,
    chained: bool,
    value: Option<i32>,
    identity: bool,
) -> Case {
    let mut values = [Some(10), Some(10), None];
    values[slot] = value;
    let ordinary = example(code, descending, values);
    let mut expected = ordinary.expected;
    if identity {
        let body = span(&expected, 12, slot);
        expected[body.start + 2..body.start + 6].copy_from_slice(&30_i32.to_le_bytes());
        let mut keys = (0..3)
            .map(|position| {
                let id = if position == slot {
                    30
                } else {
                    position as u32 + 1
                };
                let mut key = vec![127];
                key.extend((id ^ 0x8000_0000).to_be_bytes());
                key.extend(((12_u32 << 8) | position as u32).to_be_bytes());
                key
            })
            .collect::<Vec<_>>();
        keys.sort();
        let original = &ordinary.source.before[13 * PAGE..14 * PAGE];
        expected[13 * PAGE..14 * PAGE].copy_from_slice(&model::leaf(
            Some(original),
            &keys,
            0,
            0,
            0,
        ));
    }
    let final_body = expected[span(&expected, 12, slot)].to_vec();
    let forwarded = forwarding::forward(
        forwarding::source(code, forwarding::Shape::Mixed, descending, false, false),
        slot,
    );
    let mut before = forwarded.source.before;
    let final_page = if chained {
        let last = before[16 * PAGE..17 * PAGE].to_vec();
        let intermediate = span(&before, 16, 0);
        before[intermediate.start..intermediate.start + 4]
            .copy_from_slice(&(17_u32 << 8).to_le_bytes());
        let word = u16::from_le_bytes(before[16 * PAGE + 14..16 * PAGE + 16].try_into().unwrap());
        before[16 * PAGE + 14..16 * PAGE + 16].copy_from_slice(&(word | 0x4000).to_le_bytes());
        before.extend(last);
        model::map_page(
            &mut before,
            11,
            vec![model::map([12, 16, 17]), model::map([12])],
        );
        model::map_page(&mut before, 1, vec![model::map(18..512)]);
        17
    } else {
        16
    };
    expected.extend_from_slice(&before[16 * PAGE..]);
    for page in [1, 11] {
        expected[page * PAGE..(page + 1) * PAGE]
            .copy_from_slice(&before[page * PAGE..(page + 1) * PAGE]);
    }
    let root = span(&before, 12, slot);
    expected[root.clone()].copy_from_slice(&before[root]);
    let directory = 12 * PAGE + 14 + 2 * slot;
    expected[directory..directory + 2].copy_from_slice(&before[directory..directory + 2]);
    let target = span(&before, final_page, 0);
    assert_eq!(target.len(), final_body.len());
    expected[target].copy_from_slice(&final_body);
    Case {
        before,
        expected,
        values,
        final_page,
    }
}

fn updates(slot: usize, value: Option<i32>) -> [FixedScalarUpdate; 1] {
    [FixedScalarUpdate::new(
        slot,
        if slot == 2 {
            Value::Null
        } else {
            Value::Int32(10)
        },
        value.map_or(Value::Null, Value::Int32),
    )]
}

#[test]
fn qualify_selected_forwarded_sources_and_complete_expected_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (slot, value) in [(0, None), (0, Some(30)), (2, Some(30)), (2, None)] {
                for chained in [false, true] {
                    let case = case(code, descending, slot, chained, value, false);
                    qualify(&case.before, &[Some(10), Some(10), None]);
                    qualify(&case.expected, &case.values);
                    assert_eq!(case.before.len(), case.expected.len());
                    let root = span(&case.before, 12, slot);
                    assert_eq!(case.before[root.clone()], case.expected[root]);
                    for page in [1, 11, 12] {
                        assert_eq!(
                            case.before[page * PAGE..(page + 1) * PAGE],
                            case.expected[page * PAGE..(page + 1) * PAGE]
                        );
                    }
                    let at = case.final_page * PAGE;
                    assert_eq!(case.before[at..at + 16], case.expected[at..at + 16]);
                    if chained {
                        assert_eq!(
                            case.before[16 * PAGE..17 * PAGE],
                            case.expected[16 * PAGE..17 * PAGE]
                        );
                    }
                }
            }
            let case = case(code, descending, 2, true, Some(30), true);
            qualify(&case.before, &[Some(10), Some(10), None]);
            qualify(&case.expected, &case.values);
        }
    }
}

#[test]
fn selected_forwarded_fixed_payload_and_masks_keep_root_and_hidden_slot_bytes() {
    for code in [5, 6] {
        for descending in [false, true] {
            for (slot, value) in [(0, None), (0, Some(30)), (2, Some(30)), (2, None)] {
                for chained in [false, true] {
                    let case = case(code, descending, slot, chained, value, false);
                    let result = capture(&case.before)
                        .preview_ace16_fixed_updates(
                            "Items",
                            "Group",
                            &updates(slot, value),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    assert_image(result.candidate_bytes(), &case.expected);
                    assert_eq!(
                        result.changed_rows(),
                        usize::from(slot != 2 || value.is_some())
                    );
                }
            }
        }
    }
}

#[test]
fn selected_forwarded_batch_changes_multiple_physical_keys_on_one_logical_row() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = case(code, descending, 2, true, Some(30), true);
            let group = updates(2, Some(30));
            let identity = [FixedScalarUpdate::new(2, Value::Int32(3), Value::Int32(30))];
            let groups = [
                FixedColumnUpdates::new("Items", "Group", &group),
                FixedColumnUpdates::new("Items", "ID", &identity),
            ];
            let result = capture(&case.before)
                .preview_ace16_batch_updates(&groups, &mut budget(), &|| false)
                .unwrap();
            assert_image(result.candidate_bytes(), &case.expected);
            assert_eq!(result.changed_rows(), 1);
        }
    }
}

#[test]
fn selected_forwarded_stale_values_and_bad_targets_return_no_candidate() {
    let case = case(6, false, 2, false, Some(30), false);
    let stale = [FixedScalarUpdate::new(
        2,
        Value::Int32(10),
        Value::Int32(30),
    )];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Group", &stale, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
    for variant in 0..2 {
        let mut before = case.before.clone();
        if variant == 0 {
            before[16 * PAGE + 4..16 * PAGE + 8].copy_from_slice(&99_u32.to_le_bytes());
        } else {
            let target = span(&before, 16, 0);
            before[target.start..target.start + 4].copy_from_slice(&(16_u32 << 8).to_le_bytes());
            let word =
                u16::from_le_bytes(before[16 * PAGE + 14..16 * PAGE + 16].try_into().unwrap());
            before[16 * PAGE + 14..16 * PAGE + 16].copy_from_slice(&(word | 0x4000).to_le_bytes());
        }
        assert_eq!(
            capture(&before)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Group",
                    &updates(2, Some(30)),
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
}

#[test]
fn selected_forwarded_terminal_cancellation_returns_no_candidate() {
    let case = case(6, false, 2, true, Some(30), false);
    let input = updates(2, Some(30));
    let healthy = Cell::new(0);
    capture(&case.before)
        .preview_ace16_fixed_updates("Items", "Group", &input, &mut budget(), &|| {
            healthy.set(healthy.get() + 1);
            false
        })
        .unwrap();
    assert!(healthy.get() > 1);
    let polls = Cell::new(0);
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Group", &input, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == healthy.get()
            })
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
}
