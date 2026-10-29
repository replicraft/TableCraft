//! Genuine fixed C3/V0 tuple declarations and original-association whole images.
use super::{assert_image, budget, capture};
use crate::table::mutation::insert::tests::multiple_indexes::model;
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexCheckStatus, IndexConstraintViolation,
    InspectError, check_table, check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

const PAGE: usize = 4096;

type Fields = [Option<i32>; 2];

struct Case {
    before: Vec<u8>,
    expected: Vec<u8>,
    original: [Fields; 3],
    final_values: [Fields; 3],
}

fn source(code: u8, descending: bool, unique: bool, partial_only: bool) -> Case {
    let mut donor = model::source(code, false, false);
    let original: [Fields; 3] = [[Some(10), Some(100)], [Some(10), None], [None, None]];
    let final_values: [Fields; 3] = if partial_only {
        [[Some(10), Some(100)], [None, None], [None, Some(200)]]
    } else {
        [[None, None], [Some(10), None], [Some(20), Some(200)]]
    };
    let td = 10 * PAGE;
    raw::put16(&mut donor.before, td + 43, 0);
    let score = td + 137;
    donor.before[score] = 4;
    donor.before[score + 15] = 3;
    donor.before[score + 16] = 0;
    raw::put16(&mut donor.before, score + 21, 8);
    raw::put16(&mut donor.before, score + 23, 4);
    // The second physical owner genuinely binds both named fixed columns.
    donor.before[td + 250] = u8::from(!descending);
    raw::put16(&mut donor.before, td + 251, 2);
    donor.before[td + 253] = u8::from(descending);
    donor.before[td + 290] = if unique { 0x83 } else { 0x82 };
    let records = original
        .iter()
        .enumerate()
        .map(|(position, values)| {
            let mut body = 3_u16.to_le_bytes().to_vec();
            body.extend((position as i32 + 1).to_le_bytes());
            body.extend(values[0].unwrap_or(0).to_le_bytes());
            body.extend(values[1].unwrap_or(0).to_le_bytes());
            body.push(
                1 | (u8::from(values[0].is_some()) << 1) | (u8::from(values[1].is_some()) << 2),
            );
            (0, body)
        })
        .collect::<Vec<_>>();
    let mut data = raw::data_page(10, &records);
    raw::put16(&mut data, 2, 4031);
    donor.before[12 * PAGE..13 * PAGE].copy_from_slice(&data);
    let before_leaf = leaf(&original, descending, None);
    donor.before[14 * PAGE..15 * PAGE].copy_from_slice(&before_leaf);

    let mut expected = donor.before.clone();
    for (position, values) in final_values.iter().enumerate() {
        let directory = 12 * PAGE + 14 + position * 2;
        let start = 12 * PAGE
            + usize::from(u16::from_le_bytes(
                expected[directory..directory + 2].try_into().unwrap(),
            ));
        for (component, value) in values.iter().enumerate() {
            if original[position][component] != *value {
                let offset = start + 6 + component * 4;
                // Existing masked UPDATE policy changes only presence when
                // assigning NULL, retaining its otherwise ignored payload.
                if let Some(value) = value {
                    expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
                let mask = 1 << (component + 1);
                expected[start + 14] =
                    (expected[start + 14] & !mask) | if value.is_some() { mask } else { 0 };
            }
        }
    }
    expected[14 * PAGE..15 * PAGE].copy_from_slice(&leaf(
        &final_values,
        descending,
        Some(&before_leaf),
    ));
    expected[td + 79..td + 83].copy_from_slice(&30_u32.to_le_bytes());
    Case {
        before: donor.before,
        expected,
        original,
        final_values,
    }
}

fn leaf(values: &[Fields; 3], descending: bool, original: Option<&[u8]>) -> Vec<u8> {
    let mut entries = values
        .iter()
        .enumerate()
        .filter_map(|(position, values)| {
            if values == &[None, None] {
                return None;
            }
            let mut wire = Vec::new();
            for (component, value) in values.iter().enumerate() {
                let mut bytes = value.map_or_else(
                    || vec![0],
                    |value| {
                        let mut bytes = vec![127];
                        bytes.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
                        bytes
                    },
                );
                if descending == (component == 0) {
                    for byte in &mut bytes {
                        *byte = !*byte;
                    }
                }
                wire.extend(bytes);
            }
            wire.extend(((12 << 8) | position as u32).to_be_bytes());
            Some(wire)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        (&left[..left.len() - 4], &left[left.len() - 4..])
            .cmp(&(&right[..right.len() - 4], &right[right.len() - 4..]))
    });
    model::leaf(original, &entries, 0, 0, 0)
}

fn qualify(bytes: &[u8], values: &[Fields; 3]) {
    let image = capture(bytes);
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
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let report = check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(report.relationships().is_empty());
    let table = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (row, values) in table.rows().iter().zip(values) {
        assert_eq!(row[1], values[0].map_or(Value::Null, Value::Int32));
        assert_eq!(row[2], values[1].map_or(Value::Null, Value::Int32));
    }
}

fn assignments(case: &Case, component: usize) -> Vec<FixedScalarUpdate> {
    case.original
        .iter()
        .zip(&case.final_values)
        .enumerate()
        .map(|(row, (before, after))| {
            FixedScalarUpdate::new(
                row,
                before[component].map_or(Value::Null, Value::Int32),
                after[component].map_or(Value::Null, Value::Int32),
            )
        })
        .collect()
}

#[test]
fn qualify_ignored_tuple_sources_and_whole_images_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for partial_only in [false, true] {
                    let case = source(code, descending, unique, partial_only);
                    qualify(&case.before, &case.original);
                    qualify(&case.expected, &case.final_values);
                    assert_eq!(case.before.len(), case.expected.len());
                    assert_eq!(
                        case.before[13 * PAGE..14 * PAGE],
                        case.expected[13 * PAGE..14 * PAGE]
                    );
                    assert_eq!(case.before[15 * PAGE..], case.expected[15 * PAGE..]);
                }
            }
        }
    }
}

#[test]
fn ignored_tuple_batches_apply_complete_original_tuples_and_keep_partial_null_keys() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for partial_only in [false, true] {
                    let case = source(code, descending, unique, partial_only);
                    let groups = assignments(&case, 0);
                    let scores = assignments(&case, 1);
                    // Reverse written group order without changing source associations.
                    let result = capture(&case.before)
                        .preview_ace16_batch_updates(
                            &[
                                FixedColumnUpdates::new("Items", "Score", &scores),
                                FixedColumnUpdates::new("Items", "Group", &groups),
                            ],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    assert_image(result.candidate_bytes(), &case.expected);
                }
            }
        }
    }
}

#[test]
fn ignored_unique_partial_null_duplicate_is_a_real_constraint_failure() {
    let case = source(6, false, true, false);
    qualify(&case.before, &case.original);
    let updates = [FixedScalarUpdate::new(2, Value::Null, Value::Int32(10))];
    assert_eq!(
        capture(&case.before)
            .preview_ace16_fixed_updates("Items", "Group", &updates, &mut budget(), &|| false,)
            .err(),
        Some(EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey,
        )))
    );
}
