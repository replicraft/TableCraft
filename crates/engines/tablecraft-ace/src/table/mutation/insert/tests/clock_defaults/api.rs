//! Actual candidates compare against the previously qualified literal images.
use super::{checked, model, original};
use crate::{
    EditError, IncrementingAutoNumber, InsertFieldValue, InspectError, OmittedRowInsert,
    WriteBlocker,
};
use model::{Case, Reading, Row};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};

struct CountingClock {
    calls: AtomicUsize,
    first: ClockReading,
    later: ClockReading,
    failure: Option<EvalError>,
}

impl CountingClock {
    fn new(reading: Reading) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            first: ClockReading::new(reading.input()).unwrap(),
            later: ClockReading::new(Reading::EpochSix.input()).unwrap(),
            failure: None,
        }
    }

    fn count(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        let prior = self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(error) = self.failure {
            Err(error)
        } else {
            Ok(if prior == 0 { self.first } else { self.later })
        }
    }
}

pub(super) fn requests(case: &Case, rows: &[Row], omitted: bool) -> Vec<OmittedRowInsert> {
    rows.iter()
        .map(|row| {
            let values = row
                .values()
                .into_iter()
                .enumerate()
                .filter(|(position, _)| !case.generated || *position != 1)
                .map(|(position, value)| {
                    if omitted && position != 1 {
                        InsertFieldValue::Omitted
                    } else {
                        InsertFieldValue::Supplied(value)
                    }
                })
                .collect();
            OmittedRowInsert::new(values)
        })
        .collect()
}

pub(super) fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

fn run(
    case: &Case,
    requests: &[OmittedRowInsert],
    clock: &dyn Clock,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let image = original::capture(&case.before);
    if case.generated {
        let result = image.preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
            "Items",
            requests,
            IncrementingAutoNumber::new(260),
            clock,
            budget,
            cancelled,
        )?;
        assert_eq!(result.before_bytes(), case.before);
        assert_eq!(result.generated_column_number(), 1);
        assert_eq!(result.generated_values().len(), requests.len());
        for (position, value) in result.generated_values().iter().enumerate() {
            assert_eq!(*value, 261 + position as i32);
        }
        Ok(result.candidate_bytes().to_vec())
    } else {
        let result = image.preview_ace16_insert_rows_with_omissions_and_clock(
            "Items", requests, clock, budget, cancelled,
        )?;
        assert_eq!(result.before_bytes(), case.before);
        assert_eq!(result.changed_rows(), requests.len());
        Ok(result.candidate_bytes().to_vec())
    }
}

#[test]
fn one_operation_clock_matches_all_24_literal_whole_images_without_host_replay() {
    for code in [5, 6] {
        for generated in [false, true] {
            let case = model::source(code, generated, "Date()", "Now()");
            for reading in [
                Reading::LeapNoon,
                Reading::BeforeEpochSix,
                Reading::EpochSix,
                Reading::Minimum,
                Reading::Maximum,
                Reading::MidnightFirst,
            ] {
                let added = model::added(&case, reading);
                let expected = model::expected(&case, &added);
                let clock = CountingClock::new(reading);
                let after = run(
                    &case,
                    &requests(&case, &added, true),
                    &clock,
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                same(&after, &expected);
                assert_eq!(clock.count(), 1);
            }
        }
    }
}

#[test]
fn supplied_null_alias_bits_and_empty_operations_do_not_read_clock() {
    for code in [5, 6] {
        for generated in [false, true] {
            let case = model::source(code, generated, "Date()", "Now()");
            let row = Row {
                dated: Some(0xbfd0000000000000),
                id: if generated { 261 } else { 10 },
                stamped: None,
            };
            let clock = CountingClock::new(Reading::LeapNoon);
            let after = run(
                &case,
                &requests(&case, std::slice::from_ref(&row), false),
                &clock,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            same(&after, &model::expected(&case, &[row]));
            same(
                &run(&case, &[], &clock, &mut original::budget(), &|| false).unwrap(),
                &case.before,
            );
            assert_eq!(clock.count(), 0);
        }
    }
}

#[test]
fn unknown_shapes_wrong_target_and_width_refuse_before_clock_read() {
    for code in [5, 6] {
        for expression in [
            "Timer()",
            "Time()",
            "Date(1)",
            "VBA.Now()",
            "Now$()",
            "Now",
            "UnresolvedFunction()",
        ] {
            let case = model::source(code, false, expression, "Now()");
            let clock = CountingClock::new(Reading::LeapNoon);
            let added = model::added(&case, Reading::LeapNoon);
            assert_eq!(
                run(
                    &case,
                    &requests(&case, &added, true),
                    &clock,
                    &mut original::budget(),
                    &|| false
                )
                .err(),
                Some(EditError::Unsupported(WriteBlocker::Properties))
            );
            assert_eq!(clock.count(), 0);
        }
        let case = model::wrong_target_source(code);
        let clock = CountingClock::new(Reading::LeapNoon);
        let bad_target = [OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
        ])];
        assert_eq!(
            run(&case, &bad_target, &clock, &mut original::budget(), &|| {
                false
            })
            .err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
        let wrong_width = [OmittedRowInsert::new(vec![InsertFieldValue::Omitted])];
        assert_eq!(
            run(
                &case,
                &wrong_width,
                &clock,
                &mut original::budget(),
                &|| false
            )
            .err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
        assert_eq!(clock.count(), 0);
    }
}

#[test]
fn exact_generated_counter_conflict_and_exhaustion_precede_clock_callback() {
    for code in [5, 6] {
        let requests = [OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
        ])];
        let clock = CountingClock::new(Reading::LeapNoon);
        let case = model::source(code, true, "Date()", "Now()");
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                    "Items",
                    &requests,
                    IncrementingAutoNumber::new(259),
                    &clock,
                    &mut original::budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Conflict)
        );
        let case = model::maximum_counter_source(code);
        assert_eq!(
            original::capture(&case.before)
                .preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                    "Items",
                    &requests,
                    IncrementingAutoNumber::new(i32::MAX),
                    &clock,
                    &mut original::budget(),
                    &|| false
                )
                .err(),
            Some(EditError::AutoNumberExhausted)
        );
        let empty = original::capture(&case.before)
            .preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                "Items",
                &[],
                IncrementingAutoNumber::new(i32::MAX),
                &clock,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        same(empty.candidate_bytes(), &case.before);
        assert_eq!(clock.count(), 0);
    }
}

#[test]
fn callback_errors_are_typed_and_never_retried() {
    let case = model::source(6, false, "Date()", "Now()");
    let added = model::added(&case, Reading::LeapNoon);
    let limit = LimitError {
        resource: Resource::Steps,
        remaining: 0,
    };
    for (failure, expected) in [
        (
            EvalError::ClockUnavailable,
            EditError::DefaultEvaluation(EvalError::ClockUnavailable),
        ),
        (
            EvalError::Overflow,
            EditError::DefaultEvaluation(EvalError::Overflow),
        ),
        (
            EvalError::Cancelled,
            EditError::Inspect(InspectError::Cancelled),
        ),
        (
            EvalError::Allocation,
            EditError::Inspect(InspectError::Allocation),
        ),
        (
            EvalError::Limit(limit),
            EditError::Inspect(InspectError::Limit(limit)),
        ),
    ] {
        let mut clock = CountingClock::new(Reading::LeapNoon);
        clock.failure = Some(failure);
        assert_eq!(
            run(
                &case,
                &requests(&case, &added, true),
                &clock,
                &mut original::budget(),
                &|| false
            )
            .err(),
            Some(expected)
        );
        assert_eq!(clock.count(), 1);
    }
}

#[test]
fn resource_early_and_terminal_cancellation_keep_clock_reads_bounded() {
    let case = model::source(6, false, "Date()", "Now()");
    let added = model::added(&case, Reading::LeapNoon);
    let requests = requests(&case, &added, true);
    for mut budget in [
        ResourceBudget::new(0, 100_000, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
        ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
    ] {
        let clock = CountingClock::new(Reading::LeapNoon);
        assert!(matches!(
            run(&case, &requests, &clock, &mut budget, &|| false),
            Err(EditError::Inspect(InspectError::Limit(_)))
        ));
        assert_eq!(clock.count(), 0);
    }
    let clock = CountingClock::new(Reading::LeapNoon);
    assert_eq!(
        run(&case, &requests, &clock, &mut original::budget(), &|| true).err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(clock.count(), 0);

    let polls = Cell::new(0);
    let healthy = run(&case, &requests, &clock, &mut original::budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    same(&healthy, &model::expected(&case, &added));
    let terminal = polls.get();
    let count = Cell::new(0);
    let late_clock = CountingClock::new(Reading::LeapNoon);
    assert_eq!(
        run(
            &case,
            &requests,
            &late_clock,
            &mut original::budget(),
            &|| {
                count.set(count.get() + 1);
                count.get() == terminal
            }
        )
        .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(late_clock.count(), 1);
}

#[test]
fn literal_only_apis_keep_needed_clock_refusal() {
    let case = model::source(6, false, "Date()", "Now()");
    let added = model::added(&case, Reading::LeapNoon);
    assert_eq!(
        original::capture(&case.before)
            .preview_ace16_insert_rows_with_omissions(
                "Items",
                &requests(&case, &added, true),
                &mut original::budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Properties))
    );
    checked(&case.before, 2);
}

#[test]
fn mixed_supplied_null_and_omitted_literal_null_use_only_needed_clock() {
    for code in [5, 6] {
        for generated in [false, true] {
            for example in super::extra_sources::examples(code, generated) {
                let request = OmittedRowInsert::new(
                    example
                        .row
                        .values()
                        .into_iter()
                        .enumerate()
                        .filter(|(position, _)| !generated || *position != 1)
                        .map(|(position, value)| {
                            if position == 0 || (position == 2 && !example.reads_clock) {
                                InsertFieldValue::Omitted
                            } else {
                                InsertFieldValue::Supplied(value)
                            }
                        })
                        .collect(),
                );
                let clock = CountingClock::new(Reading::LeapNoon);
                let after = run(
                    &example.source,
                    &[request],
                    &clock,
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
                same(
                    &after,
                    &model::expected(&example.source, std::slice::from_ref(&example.row)),
                );
                assert_eq!(clock.count(), usize::from(example.reads_clock));
            }
        }
    }
}
