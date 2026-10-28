//! Actual INSERT candidates compose absent declarations with the stored values.
use super::{DEFINITION, PAGE, budget, capture, missing_source, model, readable};
use crate::table::mutation::insert::tests::long_value_inserts::{
    Family, initialization::model as zero,
};
use crate::{EditError, InspectError, RowInsert, WriteBlocker};
use std::cell::Cell;
use tablecraft_core::{limits::Resource, values::Value};

fn exact(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let difference = actual.iter().zip(expected).position(|(a, b)| a != b);
    assert!(
        difference.is_none(),
        "first image difference {difference:?}"
    );
}

fn request(value: Value) -> RowInsert {
    RowInsert::new(vec![Value::Int32(10), Value::Int32(100), value])
}

#[test]
fn missing_pairs_compose_exact_tail_containers_and_single_or_chained_values() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo, Family::CompressedMemo] {
            let chain = match family {
                Family::Ole => Value::Binary(vec![0x92; 8145]),
                Family::Memo | Family::CompressedMemo => Value::Text("Ā".repeat(4073)),
            };
            let cases = [
                model::external(code, family, false, false),
                model::external(code, family, true, false),
                model::external(code, family, false, true),
                model::external_value(code, family, chain, true, false),
            ];

            for case in cases {
                readable(&case.before, &zero::rows(family, None));
                assert_eq!(missing_source(&case.before, true), Ok(case.new_length));
                readable(&case.expected, &zero::rows(family, Some(&case.value)));
                zero::checked_maps(&case.expected);
                let candidate = capture(&case.before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &[request(case.value.clone())],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();

                exact(candidate.candidate_bytes(), &case.expected);
                zero::checked_maps(candidate.candidate_bytes());
            }
        }
    }
}

#[test]
fn inline_only_missing_columns_keep_the_existing_td_extent_and_maps() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo, Family::CompressedMemo] {
            let case = model::inline(code, family);
            readable(&case.before, &zero::rows(family, None));
            readable(&case.expected, &zero::rows(family, Some(&case.value)));
            let candidate = capture(&case.before)
                .preview_ace16_insert_rows("Items", &[request(case.value)], &mut budget(), &|| {
                    false
                })
                .unwrap();

            exact(candidate.candidate_bytes(), &case.expected);
            assert_eq!(candidate.candidate_bytes().len(), case.before.len());
        }
    }
}

#[test]
fn nonzero_consumed_spare_has_an_exact_before_image_and_preserved_remainder() {
    for code in [5, 6] {
        for family in [Family::Ole, Family::Memo] {
            let case = model::nonzero_spare(code, family);
            readable(&case.before, &zero::rows(family, None));
            readable(&case.expected, &zero::rows(family, Some(&case.value)));
            zero::checked_maps(&case.expected);
            let candidate = capture(&case.before)
                .preview_ace16_insert_rows("Items", &[request(case.value)], &mut budget(), &|| {
                    false
                })
                .unwrap();

            exact(candidate.candidate_bytes(), &case.expected);
            assert_eq!(
                &candidate.candidate_bytes()[DEFINITION + case.new_length..DEFINITION + PAGE],
                &case.before[DEFINITION + case.new_length..DEFINITION + PAGE],
            );
        }
    }
}

#[test]
fn an_overflowing_tail_refuses_without_allocating_a_continuation() {
    for code in [5, 6] {
        let before = model::overflow_source(code, Family::Ole);
        readable(&before, &zero::rows(Family::Ole, None));
        let result = capture(&before).preview_ace16_insert_rows(
            "Items",
            &[request(Value::Binary(vec![0x92; 65]))],
            &mut budget(),
            &|| false,
        );

        assert!(matches!(
            result,
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        ));
    }
}

#[test]
fn malformed_terminal_and_header_words_never_authorize_td_tail_growth() {
    for mutation in 0..3 {
        let mut case = model::external(6, Family::Ole, false, false);
        match mutation {
            0 => case.before[DEFINITION + case.terminal] = 0,
            1 => case.before[DEFINITION + 2] ^= 1,
            2 => case.before[DEFINITION + 8] ^= 2,
            _ => unreachable!(),
        }
        let result = capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &[request(case.value)],
            &mut budget(),
            &|| false,
        );

        assert!(result.is_err(), "malformed TD mutation {mutation}");
    }
}

#[test]
fn tail_growth_chain_refuses_at_each_pool_and_the_actual_final_callback() {
    let value = Value::Binary(vec![0x92; 8145]);
    let case = model::external_value(6, Family::Ole, value, true, false);
    readable(&case.before, &zero::rows(Family::Ole, None));
    readable(&case.expected, &zero::rows(Family::Ole, Some(&case.value)));
    zero::checked_maps(&case.expected);
    let requests = [request(case.value)];
    let polls = Cell::new(0_usize);
    let mut healthy_budget = budget();
    let healthy = capture(&case.before)
        .preview_ace16_insert_rows("Items", &requests, &mut healthy_budget, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    exact(healthy.candidate_bytes(), &case.expected);

    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let initial = budget().remaining(resource);
        let used = initial - healthy_budget.remaining(resource);
        assert!(used > 0);
        let mut limited = budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let result = capture(&case.before).preview_ace16_insert_rows(
            "Items",
            &requests,
            &mut limited,
            &|| false,
        );

        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
    }

    let observed = Cell::new(0_usize);
    let result =
        capture(&case.before).preview_ace16_insert_rows("Items", &requests, &mut budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() == polls.get()
        });

    assert!(matches!(
        result,
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(observed.get(), polls.get());
}
