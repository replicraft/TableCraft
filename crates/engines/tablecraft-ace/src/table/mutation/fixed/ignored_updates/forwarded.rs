//! Source keys retain logical identity when an untouched row is forwarded.
use super::{PAGE, assert_image, budget, capture, example, model, qualify};
use crate::table::mutation::insert::tests::ignored_index_inserts::model as forwarding;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError};
use tablecraft_core::values::Value;

struct Case {
    before: Vec<u8>,
    expected: Vec<u8>,
}

fn case(code: u8, descending: bool) -> Case {
    let ordinary = example(code, descending, [Some(30), Some(10), None]);
    let forwarded = forwarding::forward(
        forwarding::source(code, forwarding::Shape::Mixed, descending, false, false),
        2,
    );
    let before = forwarded.source.before;
    assert_eq!(before.len(), 17 * PAGE);
    let mut expected = ordinary.expected;
    expected.extend_from_slice(&before[16 * PAGE..]);
    for page in [1, 11] {
        expected[page * PAGE..(page + 1) * PAGE]
            .copy_from_slice(&before[page * PAGE..(page + 1) * PAGE]);
    }
    let directory = 12 * PAGE + 18;
    let word = u16::from_le_bytes(before[directory..directory + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0x4000);
    let start = 12 * PAGE + usize::from(word & 0x1fff);
    let end = 12 * PAGE
        + usize::from(u16::from_le_bytes(
            before[directory - 2..directory].try_into().unwrap(),
        ));
    expected[directory..directory + 2].copy_from_slice(&before[directory..directory + 2]);
    expected[start..end].copy_from_slice(&before[start..end]);
    assert_eq!(
        u32::from_le_bytes(before[start..start + 4].try_into().unwrap()),
        16 << 8
    );
    Case { before, expected }
}

#[test]
fn qualify_untouched_forwarded_omitted_roots_and_whole_images_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = case(code, descending);
            qualify(&case.before, &[Some(10), Some(10), None]);
            qualify(&case.expected, &[Some(30), Some(10), None]);
            assert_eq!(case.before.len(), case.expected.len());
            assert_eq!(case.before[16 * PAGE..], case.expected[16 * PAGE..]);
            assert_eq!(case.before[PAGE..2 * PAGE], case.expected[PAGE..2 * PAGE]);
            assert_eq!(
                case.before[11 * PAGE..12 * PAGE],
                case.expected[11 * PAGE..12 * PAGE]
            );
        }
    }
}

#[test]
fn untouched_forwarded_omitted_roots_remain_logical() {
    for code in [5, 6] {
        for descending in [false, true] {
            let case = case(code, descending);
            let updates = [FixedScalarUpdate::new(
                0,
                Value::Int32(10),
                Value::Int32(30),
            )];
            let result = capture(&case.before)
                .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false)
                .unwrap();
            assert_image(result.candidate_bytes(), &case.expected);
        }
    }
}

#[test]
fn hidden_target_index_identity_and_stale_or_conflicting_demands_refuse() {
    let case = case(6, false);
    let mut corrupted = case.before.clone();
    let mut entries = Vec::new();
    for reference in [12_u32 << 8, 16_u32 << 8] {
        let mut key = vec![127];
        key.extend((10_u32 ^ 0x8000_0000).to_be_bytes());
        key.extend(reference.to_be_bytes());
        entries.push(key);
    }
    corrupted[14 * PAGE..15 * PAGE].copy_from_slice(&model::leaf(None, &entries, 0, 0, 0));
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(30),
    )];
    assert_eq!(
        capture(&corrupted)
            .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Inspect(InspectError::Malformed))
    );

    let stale = [FixedScalarUpdate::new(0, Value::Null, Value::Null)];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Group", &stale, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Conflict)
    );
    let first = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(30),
    )];
    let conflicting = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(40),
    )];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_batch_updates(
                &[
                    FixedColumnUpdates::new("Items", "Group", &first),
                    FixedColumnUpdates::new("ITEMS", "GROUP", &conflicting),
                ],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );
}
