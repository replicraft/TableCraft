use super::{budget, model};
use crate::{
    Ace16BeforeChangePreview, BeforeChangePreviewError, BeforeEventGuardError,
    BeforeEventGuardSelector, BeforeEventIntents, EditError, FixedColumnUpdates, FixedScalarUpdate,
    InspectError, Int32Update, NativeImage, WriteBlocker,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SELECTORS: &[BeforeEventGuardSelector<'_>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn updates(late: bool) -> Vec<FixedScalarUpdate> {
    let mut values = vec![FixedScalarUpdate::new(
        model::TARGET,
        Value::Null,
        Value::Int32(99),
    )];
    if late {
        values.push(FixedScalarUpdate::new(
            model::TARGET + 1,
            Value::Null,
            Value::Int32(98),
        ));
    }
    values
}

fn run(
    before: &[u8],
    values: &[FixedScalarUpdate],
    resources: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16BeforeChangePreview, BeforeChangePreviewError> {
    capture(before).preview_ace16_before_change_updates(
        "Items", SELECTORS, "Score", values, resources, cancelled,
    )
}

fn exact_image(actual: &[u8], expected: &[u8]) {
    if actual == expected {
        return;
    }
    let differences = actual
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (left, right))| left != right)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    panic!(
        "complete image mismatch: actual length {}, expected length {}, differing bytes {}, first offsets {:?}",
        actual.len(),
        expected.len(),
        differences.len(),
        &differences[..differences.len().min(8)]
    );
}

#[test]
fn before_change_fields_bind_group_metadata_name_without_sql_identifier_parsing() {
    for code in [5, 6] {
        let case = model::source(code, false, false, false);
        let image = capture(&case.before);
        let values = updates(false);
        let prepared = image
            .prepare_ace16_before_change_fields(
                "Items",
                SELECTORS,
                BeforeEventIntents::FixedUpdate {
                    column: "Score",
                    updates: &values,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let rows = prepared.final_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ordinal(), model::TARGET);
        assert_eq!(
            rows[0].values(),
            [Value::Int32(301), Value::Int32(99), Value::Int32(301)]
        );
        assert_eq!(
            prepared
                .assignments()
                .iter()
                .map(|step| step.column())
                .collect::<Vec<_>>(),
            [1, 2]
        );
        exact_image(image.as_bytes(), &case.before);
    }
}

#[test]
fn before_change_physical_candidates_match_all_eight_prefrozen_images_and_empty_operations() {
    let mut compared = 0;
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                let case = model::source(code, descending, unique, false);
                let candidate =
                    run(&case.before, &updates(false), &mut budget(), &|| false).unwrap();
                exact_image(candidate.before_bytes(), &case.before);
                exact_image(candidate.candidate_bytes(), &case.expected);
                assert_eq!(candidate.changed_rows(), 1);
                assert_eq!(candidate.programs().len(), 1);
                assert_eq!(candidate.assignments().len(), 2);

                let empty = run(&case.before, &[], &mut budget(), &|| false).unwrap();
                exact_image(empty.before_bytes(), &case.before);
                exact_image(empty.candidate_bytes(), &case.before);
                assert_eq!(empty.changed_rows(), 0);
                assert_eq!(empty.programs().len(), 1);
                assert!(empty.assignments().is_empty());
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 8);
}

#[test]
fn before_change_physical_raise_and_stale_intents_return_no_candidate() {
    for code in [5, 6] {
        let case = model::source(code, false, false, true);
        for reverse in [false, true] {
            let mut values = updates(true);
            if reverse {
                values.reverse();
            }
            match run(&case.before, &values, &mut budget(), &|| false) {
                Err(BeforeChangePreviewError::Rejected(rejection)) => {
                    assert_eq!(rejection.number(), 17);
                    assert_eq!(rejection.description(), Some("rejected"));
                }
                Err(error) => panic!("wrong rejection: {error}"),
                Ok(_) => panic!("RaiseError returned a physical candidate"),
            }
        }
        exact_image(&case.before, &case.expected);
    }
    let case = model::source(6, false, true, false);
    let stale = [
        FixedScalarUpdate::new(model::TARGET, Value::Null, Value::Int32(99)),
        FixedScalarUpdate::new(model::TARGET + 1, Value::Int32(0), Value::Int32(98)),
    ];
    assert!(matches!(
        run(&case.before, &stale, &mut budget(), &|| false),
        Err(BeforeChangePreviewError::Preparation(
            BeforeEventGuardError::Intent(EditError::Conflict)
        ))
    ));
    let duplicate = [
        FixedScalarUpdate::new(model::TARGET, Value::Null, Value::Int32(99)),
        FixedScalarUpdate::new(model::TARGET, Value::Null, Value::Int32(98)),
    ];
    assert!(matches!(
        run(&case.before, &duplicate, &mut budget(), &|| false),
        Err(BeforeChangePreviewError::Preparation(
            BeforeEventGuardError::Intent(EditError::Conflict)
        ))
    ));

    // Existing callers never acquire this purpose-specific Extra permission,
    // including a named empty ordinary batch and the normal ACE save gate.
    assert!(matches!(
        capture(&case.before).preview_ace16_batch_updates(
            &[FixedColumnUpdates::new("Items", "Score", &[])],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::ActiveContent))
    ));
    assert!(matches!(
        capture(&case.before).plan_int32_updates(
            "Items",
            "ID",
            &[Int32Update::new(model::TARGET, 301, 302)],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::Format))
    ));
}

#[test]
fn before_change_physical_exact_pools_and_terminal_cancel_return_no_partial_result() {
    let case = model::source(6, false, true, false);
    let values = updates(false);
    let mut measured = budget();
    let initial = [
        measured.remaining(Resource::Bytes),
        measured.remaining(Resource::Rows),
        measured.remaining(Resource::Steps),
    ];
    let polls = Cell::new(0);
    let healthy = run(&case.before, &values, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    exact_image(healthy.candidate_bytes(), &case.expected);
    let used = [
        initial[0] - measured.remaining(Resource::Bytes),
        initial[1] - measured.remaining(Resource::Rows),
        initial[2] - measured.remaining(Resource::Steps),
    ];
    assert!(used.iter().all(|value| *value > 0));
    let exact = run(
        &case.before,
        &values,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    exact_image(exact.candidate_bytes(), &case.expected);
    for pool in 0..3 {
        let mut limit = used;
        limit[pool] -= 1;
        assert!(matches!(
            run(
                &case.before,
                &values,
                &mut ResourceBudget::new(limit[0], limit[1], limit[2]),
                &|| false
            ),
            Err(BeforeChangePreviewError::Preparation(
                BeforeEventGuardError::Inspect(InspectError::Limit(_))
            )) | Err(BeforeChangePreviewError::Edit(EditError::Inspect(
                InspectError::Limit(_)
            )))
        ));
    }
    let final_poll = polls.get();
    polls.set(0);
    assert!(matches!(
        run(&case.before, &values, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        }),
        Err(BeforeChangePreviewError::Preparation(
            BeforeEventGuardError::Inspect(InspectError::Cancelled)
        )) | Err(BeforeChangePreviewError::Edit(EditError::Inspect(
            InspectError::Cancelled
        )))
    ));
}
