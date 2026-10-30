//! Finite pre-fix diagnostics: healthy exact images first, all bad outcomes retained.
use super::{
    model::{self, Family},
    selected_orphan,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    Ace16WritePreview, EditError, FixedColumnUpdates, FixedScalarUpdate, InspectError, Int32Update,
    NativeEditPlan, NativeImage, WriteBlocker,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const WORK_BYTES: u64 = 256 * 1024 * 1024;
const WORK_ROWS: u64 = 100_000;
const WORK_STEPS: u64 = 100_000_000;

#[derive(Clone, Copy, Debug)]
enum Route {
    Normal,
    Int32Preview,
    Fixed,
    Batch,
}

impl Route {
    fn for_header(code: u8) -> &'static [Self] {
        if code == 1 {
            &[Self::Normal]
        } else {
            &[Self::Int32Preview, Self::Fixed, Self::Batch]
        }
    }
}

enum Candidate {
    Normal(NativeEditPlan),
    Preview(Ace16WritePreview),
}

impl Candidate {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Normal(plan) => plan.after_bytes(),
            Self::Preview(plan) => plan.candidate_bytes(),
        }
    }
    fn changed(&self) -> usize {
        match self {
            Self::Normal(plan) => plan.changed_rows(),
            Self::Preview(plan) => plan.changed_rows(),
        }
    }
}

fn run(bytes: &[u8], route: Route, empty: bool) -> Result<Candidate, EditError> {
    run_with(
        bytes,
        route,
        empty,
        &mut ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS),
        &|| false,
    )
}

fn run_with(
    bytes: &[u8],
    route: Route,
    empty: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Candidate, EditError> {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        budget,
        cancelled,
    )?;
    let integers = if empty {
        vec![]
    } else {
        vec![Int32Update::new(0, 10, 11)]
    };
    let scalars = if empty {
        vec![]
    } else {
        vec![FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(11),
        )]
    };
    match route {
        Route::Normal => image
            .plan_int32_updates("Items", "Score", &integers, budget, cancelled)
            .map(Candidate::Normal),
        Route::Int32Preview => image
            .preview_ace16_int32_updates("Items", "Score", &integers, budget, cancelled)
            .map(Candidate::Preview),
        Route::Fixed => image
            .preview_ace16_fixed_updates("Items", "Score", &scalars, budget, cancelled)
            .map(Candidate::Preview),
        Route::Batch => image
            .preview_ace16_batch_updates(
                &[FixedColumnUpdates::new("Items", "Score", &scalars)],
                budget,
                cancelled,
            )
            .map(Candidate::Preview),
    }
}

fn exact(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    let offset = actual.iter().zip(expected).position(|(a, b)| a != b);
    assert!(offset.is_none(), "healthy first differing byte {offset:?}");
}

#[test]
fn standard_fixed_routes_collect_all_body_and_selected_orphan_probe_outcomes() {
    let families = [Family::SameTable, Family::Foreign, Family::Orphan];
    let mut healthy = 0;
    for code in [1, 5, 6] {
        for family in families {
            let case = model::case(code, family);
            for &route in Route::for_header(code) {
                for empty in [false, true] {
                    let candidate = run(&case.before, route, empty).unwrap_or_else(|error| {
                        panic!("healthy {code}/{family:?}/{route:?}/empty={empty}: {error}")
                    });
                    exact(
                        candidate.bytes(),
                        if empty { &case.before } else { &case.expected },
                    );
                    assert_eq!(candidate.changed(), usize::from(!empty));
                    healthy += 1;
                }
            }
        }
    }
    assert_eq!(healthy, 42);
    println!("HEALTHY_COMPLETE_IMAGES\t{healthy}");

    let mut outcomes = 0;
    let mut unexpected = 0;
    for code in [1, 5, 6] {
        for family in families {
            let case = model::case(code, family);
            for &route in Route::for_header(code) {
                for empty in [false, true] {
                    match run(&case.bad, route, empty) {
                        Ok(candidate) => {
                            unexpected += 1;
                            println!(
                                "BODY\t{code}\t{family:?}\t{route:?}\tempty={empty}\tACCEPTED\tchanged={}",
                                candidate.changed()
                            );
                        }
                        Err(error) => {
                            if !matches!(&error, EditError::Unsupported(WriteBlocker::RowLayout)) {
                                unexpected += 1;
                            }
                            println!(
                                "BODY\t{code}\t{family:?}\t{route:?}\tempty={empty}\tREFUSED\t{error}"
                            );
                        }
                    }
                    outcomes += 1;
                }
            }
        }
        let source = selected_orphan::source(code);
        for &route in Route::for_header(code) {
            for empty in [false, true] {
                match run(&source, route, empty) {
                    Ok(candidate) => {
                        unexpected += 1;
                        println!(
                            "SELECTED_ORPHAN\t{code}\t{route:?}\tempty={empty}\tACCEPTED\tchanged={}",
                            candidate.changed()
                        );
                    }
                    Err(error) => {
                        if !matches!(&error, EditError::Unsupported(WriteBlocker::IndexedColumn)) {
                            unexpected += 1;
                        }
                        println!(
                            "SELECTED_ORPHAN\t{code}\t{route:?}\tempty={empty}\tREFUSED\t{error}"
                        );
                    }
                }
                outcomes += 1;
            }
        }
    }
    assert_eq!(outcomes, 56);
    println!("ALL_BAD_OUTCOMES\t{outcomes}\tUNEXPECTED\t{unexpected}");
    assert_eq!(
        unexpected, 0,
        "all pre-fix outcomes collected before this final diagnostic assertion"
    );
}

#[test]
fn foreign_index_scan_full_operations_pay_exact_pools_and_final_cancellation() {
    for (code, route) in [(1, Route::Normal), (6, Route::Batch)] {
        let case = model::case(code, Family::Foreign);
        let mut measured = ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS);
        let calls = Cell::new(0);
        let candidate = run_with(&case.before, route, false, &mut measured, &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        exact(candidate.bytes(), &case.expected);
        drop(candidate);

        // The real operation includes immutable capture, every local physical
        // owner scan, fixed preparation and final checks under this one budget.
        for (pool, available) in [
            (Resource::Bytes, WORK_BYTES),
            (Resource::Rows, WORK_ROWS),
            (Resource::Steps, WORK_STEPS),
        ] {
            let required = available - measured.remaining(pool);
            assert!(required > 0);
            for (limit, succeeds) in [(required, true), (required - 1, false)] {
                let (bytes, rows, steps) = match pool {
                    Resource::Bytes => (limit, WORK_ROWS, WORK_STEPS),
                    Resource::Rows => (WORK_BYTES, limit, WORK_STEPS),
                    Resource::Steps => (WORK_BYTES, WORK_ROWS, limit),
                };
                let result = run_with(
                    &case.before,
                    route,
                    false,
                    &mut ResourceBudget::new(bytes, rows, steps),
                    &|| false,
                );
                if succeeds {
                    exact(result.unwrap().bytes(), &case.expected);
                } else {
                    assert!(
                        matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == pool)
                    );
                }
            }
        }

        let stop = calls.get();
        let calls = Cell::new(0);
        assert!(matches!(
            run_with(
                &case.before,
                route,
                false,
                &mut ResourceBudget::new(WORK_BYTES, WORK_ROWS, WORK_STEPS),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            ),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert_eq!(calls.get(), stop);
    }
}
