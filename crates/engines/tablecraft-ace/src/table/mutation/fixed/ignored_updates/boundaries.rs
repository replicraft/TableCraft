//! Independent retained-node, Required/unique and malformed-source controls.
use super::{PAGE, assert_image, budget, capture, example, leaf, model, qualify, requests};
use crate::{EditError, FixedScalarUpdate, IndexConstraintViolation, InspectError, WriteBlocker};
use tablecraft_core::values::Value;
use tablecraft_testkit::access::multiple_indexes as frames;

struct NodeCase {
    before: Vec<u8>,
    expected: Vec<u8>,
    values: [Option<i32>; 3],
}

fn node_case(code: u8, descending: bool, mixed: bool) -> NodeCase {
    let values = if mixed {
        [None, Some(10), Some(20)]
    } else {
        [Some(10), Some(10), Some(20)]
    };
    let case = example(code, descending, values);
    let mut before = case.source.before.clone();
    let mut expected = case.expected;
    before.resize(18 * PAGE, 0);
    expected.resize(18 * PAGE, 0);

    let original = entries(&[Some(10), Some(10), None], descending);
    let final_keys = entries(&values, descending);
    let left = frames::leaf(None, &original[..1], 0, 0, 17);
    let right = frames::leaf(None, &original[1..], 0, 16, 0);
    let root = frames::root(&before[14 * PAGE..15 * PAGE], 16, 17, &original[0]);
    before[14 * PAGE..15 * PAGE].copy_from_slice(&root);
    before[16 * PAGE..17 * PAGE].copy_from_slice(&left);
    before[17 * PAGE..18 * PAGE].copy_from_slice(&right);
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&frames::root(&root, 16, 17, &final_keys[0]));
    expected[16 * PAGE..17 * PAGE].copy_from_slice(&frames::leaf(
        Some(&left),
        &final_keys[..1],
        0,
        0,
        17,
    ));
    expected[17 * PAGE..18 * PAGE].copy_from_slice(&frames::leaf(
        Some(&right),
        &final_keys[1..],
        0,
        16,
        0,
    ));
    for image in [&mut before, &mut expected] {
        frames::map_page(image, 1, vec![frames::map(18..512)]);
        frames::map_page(
            image,
            15,
            vec![frames::map([13]), frames::map([14, 16, 17])],
        );
    }
    NodeCase {
        before,
        expected,
        values,
    }
}

fn entries(values: &[Option<i32>; 3], descending: bool) -> Vec<Vec<u8>> {
    let mut keys = Vec::new();
    for (slot, value) in values.iter().enumerate() {
        if let Some(value) = value {
            let mut wire = vec![127];
            wire.extend(((*value as u32) ^ 0x8000_0000).to_be_bytes());
            if descending {
                for byte in &mut wire {
                    *byte = !*byte;
                }
            }
            wire.extend(((12 << 8) | slot as u32).to_be_bytes());
            keys.push(wire);
        }
    }
    keys.sort();
    keys
}

fn constrained_source(code: u8, required: bool) -> Vec<u8> {
    let mut source = frames::source(code, false, !required);
    source.before[10 * PAGE + 290] = if required { 0x8a } else { 0x83 };
    if required {
        source.rows[2].0.group = Some(30);
        let directory = 12 * PAGE + 18;
        let start = 12 * PAGE
            + usize::from(u16::from_le_bytes(
                source.before[directory..directory + 2].try_into().unwrap(),
            ));
        let end = 12 * PAGE
            + usize::from(u16::from_le_bytes(
                source.before[directory - 2..directory].try_into().unwrap(),
            ));
        source.before[start + 6..start + 10].copy_from_slice(&30_i32.to_le_bytes());
        source.before[end - 1] |= 2;
    }
    let values = source
        .rows
        .iter()
        .map(|(row, _)| row.group)
        .collect::<Vec<_>>();
    let root = leaf(&source, &values, None);
    source.before[14 * PAGE..15 * PAGE].copy_from_slice(&root);
    source.before
}

fn primary_alias_source(code: u8, ordinary_alias_first: bool) -> Vec<u8> {
    let mut source = model::ignored_null_source(code).before;
    source[10 * PAGE + 238] = 0x8b;
    if ordinary_alias_first {
        let first = 10 * PAGE + 296;
        let second = first + 28;
        let primary = source[first..second].to_vec();
        source.copy_within(second..second + 28, first);
        source[second..second + 28].copy_from_slice(&primary);
    }
    source
}

#[test]
fn qualify_retained_node_and_constrained_ignored_update_sources_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            for mixed in [false, true] {
                let case = node_case(code, descending, mixed);
                qualify(&case.before, &[Some(10), Some(10), None]);
                qualify(&case.expected, &case.values);
                assert_eq!(case.before.len(), case.expected.len());
                assert_eq!(
                    case.before[15 * PAGE..16 * PAGE],
                    case.expected[15 * PAGE..16 * PAGE]
                );
                assert_eq!(case.before[..10 * PAGE], case.expected[..10 * PAGE]);
                assert_eq!(
                    case.before[11 * PAGE..12 * PAGE],
                    case.expected[11 * PAGE..12 * PAGE]
                );
            }
        }
        qualify(
            &constrained_source(code, false),
            &[Some(10), Some(20), None],
        );
        qualify(
            &constrained_source(code, true),
            &[Some(10), Some(10), Some(30)],
        );
        for ordinary_alias_first in [false, true] {
            let source = primary_alias_source(code, ordinary_alias_first);
            qualify(&source, &[Some(10), Some(10), None]);
            let image = capture(&source);
            let checked = crate::check_table(
                &mut image.reader(),
                tablecraft_model::CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.indexes()[0].index().data_number(), 0);
            assert_eq!(checked.indexes()[1].index().data_number(), 0);
            assert_eq!(
                checked.indexes()[0].index().native_type(),
                u8::from(!ordinary_alias_first)
            );
            assert_eq!(
                checked.indexes()[1].index().native_type(),
                u8::from(ordinary_alias_first)
            );
        }
    }
}

#[test]
fn ignored_primary_alias_order_never_grants_secondary_update_admission() {
    for code in [5, 6] {
        for ordinary_alias_first in [false, true] {
            let source = primary_alias_source(code, ordinary_alias_first);
            for (column, updates) in [
                (
                    "ID",
                    vec![FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(4))],
                ),
                ("ID", Vec::new()),
                (
                    "Group",
                    vec![FixedScalarUpdate::new(
                        0,
                        Value::Int32(10),
                        Value::Int32(30),
                    )],
                ),
            ] {
                assert_eq!(
                    capture(&source)
                        .preview_ace16_fixed_updates(
                            "Items",
                            column,
                            &updates,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
                );
            }
        }
    }
}

#[test]
fn retained_node_ignored_updates_rederive_bounds_without_changing_any_map_or_height() {
    for code in [5, 6] {
        for descending in [false, true] {
            for mixed in [false, true] {
                let case = node_case(code, descending, mixed);
                let result = capture(&case.before)
                    .preview_ace16_fixed_updates(
                        "Items",
                        "Group",
                        &requests(&case.values),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_image(result.candidate_bytes(), &case.expected);
            }
        }
    }
}

#[test]
fn required_precedes_omission_and_unique_checks_real_added_membership() {
    for code in [5, 6] {
        let required = constrained_source(code, true);
        let updates = [FixedScalarUpdate::new(2, Value::Int32(30), Value::Null)];
        assert_eq!(
            capture(&required)
                .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false,)
                .err(),
            Some(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::NullKey,
            )))
        );
        let unique = constrained_source(code, false);
        let updates = [FixedScalarUpdate::new(2, Value::Null, Value::Int32(20))];
        assert_eq!(
            capture(&unique)
                .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false,)
                .err(),
            Some(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey,
            )))
        );
    }
}

#[test]
fn node_empty_child_transition_stays_explicit_and_malformed_eligible_sources_refuse() {
    let case = node_case(6, false, false);
    let updates = [FixedScalarUpdate::new(0, Value::Int32(10), Value::Null)];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );

    let case = example(6, false, [Some(10), Some(10), Some(20)]);
    let original = entries(&[Some(10), Some(10), None], false);
    let mut with_null = original.clone();
    with_null.push([vec![0], ((12_u32 << 8) | 2).to_be_bytes().to_vec()].concat());
    with_null.sort();
    let mut phantom = original.clone();
    phantom[1][5..9].copy_from_slice(&((12_u32 << 8) | 3).to_be_bytes());
    for entries in [original[..1].to_vec(), with_null, phantom] {
        let mut source = case.source.before.clone();
        source[14 * PAGE..15 * PAGE].copy_from_slice(&frames::leaf(None, &entries, 0, 0, 0));
        assert_eq!(
            capture(&source)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Group",
                    &requests(&case.final_values),
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(EditError::Inspect(InspectError::Malformed))
        );
    }
}
