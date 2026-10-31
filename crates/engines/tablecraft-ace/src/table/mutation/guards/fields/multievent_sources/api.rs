use super::{budget, model};
use crate::{
    BeforeChangePreviewError, BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents,
    EditError, FixedScalarUpdate, InspectError, NativeImage, RowInsert,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn selectors(case: &model::Case) -> Vec<BeforeEventGuardSelector<'_>> {
    case.selection
        .iter()
        .map(|index| BeforeEventGuardSelector::new(3, &case.properties[*index].0))
        .collect()
}

fn updates() -> [FixedScalarUpdate; 1] {
    [FixedScalarUpdate::new(300, Value::Null, Value::Int32(99))]
}

#[test]
fn complete_collection_dispatches_only_change_and_matches_all_frozen_images() {
    let mut candidates = 0;
    for code in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(code, recipe);
            if !case.changed {
                continue;
            }
            let selection = selectors(&case);
            let image = capture(&case.images.before);
            let candidate = image
                .preview_ace16_before_change_updates(
                    "Items",
                    &selection,
                    "Score",
                    &updates(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(
                candidate.candidate_bytes() == case.images.expected,
                "{code} {recipe:?}: full image"
            );
            assert_eq!(candidate.changed_rows(), 1);

            let empty = capture(&case.images.before)
                .preview_ace16_before_change_updates(
                    "Items",
                    &selection,
                    "Score",
                    &[],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(
                empty.candidate_bytes() == case.images.before,
                "{code} {recipe:?}: empty image"
            );
            assert_eq!(empty.changed_rows(), 0);
            candidates += 1;
        }
    }
    assert_eq!(candidates, 10);
}

#[test]
fn invalid_dormant_and_incomplete_definitions_refuse_even_empty_intents() {
    let mut refusals = 0;
    for code in [5, 6] {
        for recipe in [
            model::Recipe::DormantCurrent,
            model::Recipe::DormantSetField,
            model::Recipe::UnknownEvent,
            model::Recipe::IncompleteSelection,
            model::Recipe::DormantOpaque,
            model::Recipe::DuplicateSelector,
        ] {
            let case = model::source(code, recipe);
            let selection = selectors(&case);
            let values = updates();
            for requested in [&values[..], &[][..]] {
                let result = capture(&case.images.before).preview_ace16_before_change_updates(
                    "Items",
                    &selection,
                    "Score",
                    requested,
                    &mut budget(),
                    &|| false,
                );
                assert!(
                    matches!(result, Err(BeforeChangePreviewError::Preparation(_))),
                    "{code} {recipe:?}"
                );
                refusals += 1;
            }
        }
    }
    assert_eq!(refusals, 24);
}

#[test]
fn active_raise_discards_rows_and_existing_public_guard_context_stays_closed() {
    for code in [5, 6] {
        let case = model::source(code, model::Recipe::ActiveChangeRaise);
        let selection = selectors(&case);
        let result = capture(&case.images.before).preview_ace16_before_change_updates(
            "Items",
            &selection,
            "Score",
            &updates(),
            &mut budget(),
            &|| false,
        );
        assert!(
            matches!(result, Err(BeforeChangePreviewError::Rejected(ref value)) if value.number() == 17)
        );

        let guarded = model::source(code, model::Recipe::Separate);
        let image = capture(&guarded.images.before);
        let selection = selectors(&guarded);
        let values = updates();
        assert!(
            image
                .prepare_ace16_before_event_guards(
                    "Items",
                    &selection,
                    BeforeEventIntents::FixedUpdate {
                        column: "Score",
                        updates: &values
                    },
                    &mut budget(),
                    &|| false,
                )
                .is_err()
        );
    }
}

#[test]
fn complete_preflight_is_paid_and_final_cancellation_returns_no_candidate() {
    let case = model::source(6, model::Recipe::DormantDeleteRaise);
    let selection = selectors(&case);
    let values = updates();
    let run = |resources: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
        capture(&case.images.before).preview_ace16_before_change_updates(
            "Items", &selection, "Score", &values, resources, cancelled,
        )
    };
    let polls = Cell::new(0);
    let mut resources = budget();
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let initial = pools.map(|pool| resources.remaining(pool));
    let healthy = run(&mut resources, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert!(healthy.candidate_bytes() == case.images.expected);
    let consumed =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - resources.remaining(pools[index]));
    let exact = run(
        &mut ResourceBudget::new(consumed[0], consumed[1], consumed[2]),
        &|| false,
    )
    .unwrap();
    assert!(exact.candidate_bytes() == case.images.expected);
    for pool in 0..3 {
        assert!(consumed[pool] > 0);
        let mut limits = consumed;
        limits[pool] -= 1;
        assert!(matches!(
            run(
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
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
        run(&mut budget(), &|| {
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

#[test]
fn logical_insert_keeps_dormant_delete_old_separate_from_current_insert() {
    for code in [5, 6] {
        let case = model::source(code, model::Recipe::DormantDeleteRaise);
        let image = capture(&case.images.before);
        let selection = selectors(&case);
        // This ID already exists in the source. Logical preparation is not
        // physical insertion or constraint permission, and creates no candidate.
        let inserts = [RowInsert::new(vec![
            Value::Int32(301),
            Value::Null,
            Value::Int32(99),
        ])];
        let prepared = image
            .prepare_ace16_before_change_fields(
                "Items",
                &selection,
                BeforeEventIntents::Insert(&inserts),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(prepared.rejection().is_none());
        let rows = prepared.final_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ordinal(), 0);
        assert_eq!(
            rows[0].values(),
            &[Value::Int32(301), Value::Int32(99), Value::Int32(301)]
        );
    }
}

#[test]
fn coverage_collection_does_not_widen_legacy_selected_event_guard_compilation() {
    for code in [5, 6] {
        let case = model::source(code, model::Recipe::DormantDeleteRaise);
        let selection = selectors(&case);
        let image = capture(&case.images.before);
        let coverage = image
            .inspect_table_macro_coverage(
                CatalogScope::new(7).object_id(10),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let updates = updates();
        let fields = coverage
            .prepare_before_change_fields(
                &selection,
                BeforeEventIntents::FixedUpdate {
                    column: "Score",
                    updates: &updates,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(fields.rejection().is_none());
        assert_eq!(
            fields.final_rows().unwrap()[0].values(),
            [Value::Int32(301), Value::Int32(99), Value::Int32(301)]
        );
        assert_eq!(fields.programs().len(), 2);

        // BeforeChange is dormant for this explicit Delete. The unchanged guard
        // compiler refuses that context; complete property selection alone does
        // not introduce CompleteFields semantic dispatch into the guard method.
        let intent = BeforeEventIntents::DeleteOrdinals(&[]);
        let direct = image
            .prepare_ace16_before_event_guards("Items", &selection, intent, &mut budget(), &|| {
                false
            })
            .err();
        let bound = coverage
            .prepare_before_event_guards(&selection, intent, &mut budget(), &|| false)
            .err();
        assert_eq!(
            direct,
            Some(BeforeEventGuardError::Unsupported(
                crate::BeforeGuardUnsupported::Context
            ))
        );
        assert_eq!(bound, direct);
        assert_eq!(image.as_bytes(), case.images.before);
    }
}
