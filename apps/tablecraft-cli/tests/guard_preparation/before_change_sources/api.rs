use super::{CASES, expected, inspect, physical_roles};
use std::cell::Cell;
use tablecraft_access::{
    NativeEditError,
    engineering::{
        ScalarUpdateMode, UpdateMode,
        guards::before_change::{self, BeforeChangeBackendError},
        sql,
    },
};
use tablecraft_ace::{
    Ace16BeforeChangePreview, BeforeChangePreviewError, BeforeEventGuardSelector, EditError,
    WriteBlocker,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{UpdateError, UpdatePlan};
use tablecraft_sql::{ParseLimits, parse_query};

const SELECTORS: &[BeforeEventGuardSelector<'_>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];

fn run(
    before: &[u8],
    text: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<Ace16BeforeChangePreview>, UpdateError<BeforeChangeBackendError>> {
    let image = inspect::source(before);
    let query = parse_query(text, ParseLimits::default()).unwrap();
    before_change::preview_update(image, query.query(), selectors, budget, None, cancelled)
}

#[test]
fn before_change_shared_all_eight_source_expected_results_are_exact() {
    let mut compared = 0;
    for code in [5, 6] {
        for (text, body, score) in CASES {
            let (before, _, _) = physical_roles::source(code, body);
            let result = run(&before, text, SELECTORS, &mut inspect::budget(), &|| false);
            if score.is_none() {
                assert!(matches!(
                    result,
                    Err(UpdateError::Backend(BeforeChangeBackendError::Preview(
                        BeforeChangePreviewError::Rejected(_)
                    )))
                ));
            } else {
                let plan = result.unwrap();
                assert_eq!(plan.matched_rows(), 1);
                let candidate = plan.into_backend();
                assert_eq!(candidate.before_bytes(), before);
                assert_eq!(candidate.candidate_bytes(), expected(&before, score));
                assert_eq!(candidate.changed_rows(), usize::from(score != Some(20)));
                assert_eq!(candidate.programs().len(), 1);
                assert_eq!(candidate.assignments().len(), 1);
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 8);
}

#[test]
fn before_change_shared_empty_selection_still_compiles_original_program() {
    for code in [5, 6] {
        let (_, body, _) = CASES[0];
        let (before, _, _) = physical_roles::source(code, body);
        let plan = run(
            &before,
            "UPDATE Items SET Score=99 WHERE Id=999",
            SELECTORS,
            &mut inspect::budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), 0);
        let candidate = plan.into_backend();
        assert_eq!(candidate.before_bytes(), before);
        assert_eq!(candidate.candidate_bytes(), before);
        assert_eq!(candidate.changed_rows(), 0);
        assert_eq!(candidate.programs().len(), 1);
        assert!(candidate.assignments().is_empty());
    }
}

#[test]
fn before_change_shared_late_raise_discards_earlier_proposed_rows() {
    for code in [5, 6] {
        let (text, body, _) = CASES[3];
        let (before, _, _) = physical_roles::source(code, body);
        match run(&before, text, SELECTORS, &mut inspect::budget(), &|| false) {
            Err(UpdateError::Backend(BeforeChangeBackendError::Preview(
                BeforeChangePreviewError::Rejected(rejection),
            ))) => {
                assert_eq!(rejection.number(), 17);
                assert_eq!(rejection.description(), Some("rejected"));
            }
            Err(error) => panic!("wrong rejection: {error}"),
            Ok(_) => panic!("RaiseError exposed a candidate"),
        }
    }
}

#[test]
fn before_change_shared_original_selectors_remain_mandatory_even_for_empty_rows() {
    let (_, body, _) = CASES[0];
    let (before, _, _) = physical_roles::source(6, body);
    let missing = [BeforeEventGuardSelector::new(3, "Other")];
    let duplicate = [SELECTORS[0], SELECTORS[0]];
    for selectors in [missing.as_slice(), duplicate.as_slice(), &[]] {
        assert!(
            run(
                &before,
                "UPDATE Items SET Score=99 WHERE Id=999",
                selectors,
                &mut inspect::budget(),
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn before_change_shared_old_update_service_still_refuses_active_content() {
    let (text, body, _) = CASES[0];
    let (before, _, _) = physical_roles::source(6, body);
    let query = parse_query(text, ParseLimits::default()).unwrap();
    assert!(matches!(
        sql::preview_update(
            inspect::source(&before),
            query.query(),
            ScalarUpdateMode::Fixed(UpdateMode::Fitted),
            &mut inspect::budget(),
            None,
            &|| false,
        ),
        Err(UpdateError::Backend(NativeEditError::Edit(
            EditError::Unsupported(WriteBlocker::ActiveContent)
        )))
    ));
}

#[test]
fn before_change_shared_closed_statement_shape_remains_existing_single_update() {
    for text in [
        "UPDATE Items SET Score=99, Id=7",
        "DELETE FROM Items WHERE Id=1",
        "INSERT INTO Items(Id,Score) VALUES(4,40)",
    ] {
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(sql::local_update_table(query.query()).is_err());
    }
}

#[test]
fn before_change_shared_late_sql_failure_exposes_no_candidate() {
    let (_, body, _) = CASES[0];
    let (before, _, _) = physical_roles::source(6, body);
    assert!(
        run(
            &before,
            "UPDATE Items SET Score=1/(Id-2) WHERE Id<=2",
            SELECTORS,
            &mut inspect::budget(),
            &|| false,
        )
        .is_err()
    );
}

#[test]
fn before_change_shared_cumulative_pools_and_final_cancellation_refuse_results() {
    let (text, body, _) = CASES[0];
    let (before, _, _) = physical_roles::source(6, body);
    let mut measured = inspect::budget();
    run(&before, text, SELECTORS, &mut measured, &|| false).unwrap();
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let used = inspect::budget().remaining(resource) - measured.remaining(resource);
        assert!(used > 0);
        for limit in [used - 1, used] {
            let base = inspect::budget();
            let mut budget = ResourceBudget::new(
                if resource == Resource::Bytes {
                    limit
                } else {
                    base.remaining(Resource::Bytes)
                },
                if resource == Resource::Rows {
                    limit
                } else {
                    base.remaining(Resource::Rows)
                },
                if resource == Resource::Steps {
                    limit
                } else {
                    base.remaining(Resource::Steps)
                },
            );
            assert_eq!(
                run(&before, text, SELECTORS, &mut budget, &|| false).is_ok(),
                limit == used
            );
        }
    }
    let calls = Cell::new(0);
    run(&before, text, SELECTORS, &mut inspect::budget(), &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let last = calls.get();
    calls.set(0);
    assert!(
        run(&before, text, SELECTORS, &mut inspect::budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() == last
        })
        .is_err()
    );
    assert_eq!(calls.get(), last);
}
