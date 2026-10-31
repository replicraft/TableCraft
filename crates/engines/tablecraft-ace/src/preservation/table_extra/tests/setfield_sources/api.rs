use super::super::{budget, guard_selector_sources::model as selectors};
use super::{cases, model};
use crate::{
    BeforeChangeFieldPreparation, BeforeEventGuardError, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardUnsupported, EditError, FixedScalarUpdate, InspectError,
    NativeImage, RowInsert,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const SELECTED: &[BeforeEventGuardSelector<'_>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];
const TARGETS: &[&[usize]] = &[
    &[1],
    &[0, 1, 0],
    &[1, 0],
    &[1],
    &[1],
    &[1],
    &[],
    &[1, 1],
    &[1],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
];

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn prepare<'a>(
    image: &'a NativeImage,
    case: &cases::Case,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BeforeChangeFieldPreparation<'a>, BeforeEventGuardError> {
    if let Some(original) = case.original {
        let updates = original
            .iter()
            .zip(case.proposed)
            .enumerate()
            .map(|(ordinal, (before, proposed))| {
                FixedScalarUpdate::new(ordinal, Value::Int32(before[1]), Value::Int32(proposed[1]))
            })
            .collect::<Vec<_>>();
        image.prepare_ace16_before_change_fields(
            "Items",
            SELECTED,
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &updates,
            },
            budget,
            cancelled,
        )
    } else {
        let inserts = case
            .proposed
            .iter()
            .map(|row| RowInsert::new(row.iter().copied().map(Value::Int32).collect()))
            .collect::<Vec<_>>();
        image.prepare_ace16_before_change_fields(
            "Items",
            SELECTED,
            BeforeEventIntents::Insert(&inserts),
            budget,
            cancelled,
        )
    }
}

#[test]
fn before_change_fields_match_all_frozen_program_and_final_row_cases() {
    let mut checked = 0;
    for (index, case) in cases::CASES.iter().enumerate() {
        for code in [5, 6] {
            let (bytes, _, _) = model::source(code, "BeforeChange", case.body);
            let image = capture(&bytes);
            let result = prepare(&image, case, &mut budget(), &|| false);
            match case.outcome {
                "pass" => {
                    let result =
                        result.unwrap_or_else(|error| panic!("{} / {code}: {error}", case.name));
                    assert!(result.rejection().is_none());
                    let rows = result.final_rows().unwrap();
                    let expected = case.final_rows.unwrap();
                    assert_eq!(rows.len(), expected.len());
                    for (ordinal, (row, expected)) in rows.iter().zip(expected).enumerate() {
                        assert_eq!(row.ordinal(), ordinal);
                        let values = expected
                            .iter()
                            .map(|value| value.map_or(Value::Null, Value::Int32))
                            .collect::<Vec<_>>();
                        assert_eq!(row.values(), values, "{} / {code}", case.name);
                    }
                    assert_eq!(
                        result
                            .assignments()
                            .iter()
                            .map(|step| step.column())
                            .collect::<Vec<_>>(),
                        TARGETS[index]
                    );
                    assert_eq!(result.programs().len(), 1);
                    assert_eq!(result.table_object_id(), CatalogScope::new(7).object_id(10));
                    assert_eq!(result.source_bytes(), bytes);
                }
                "raised" => {
                    let result =
                        result.unwrap_or_else(|error| panic!("{} / {code}: {error}", case.name));
                    assert!(result.final_rows().is_none());
                    let rejection = result.rejection().unwrap();
                    assert_eq!(rejection.number(), 17);
                    assert_eq!(rejection.description(), Some("rejected"));
                    assert_eq!(
                        result
                            .assignments()
                            .iter()
                            .map(|step| step.column())
                            .collect::<Vec<_>>(),
                        TARGETS[index]
                    );
                }
                "argument_type" => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::ArgumentType),
                    "{}",
                    case.name
                ),
                "unsupported_context" => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context
                    )),
                    "{}",
                    case.name
                ),
                "unsupported_expression" => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Expression
                    )),
                    "{}",
                    case.name
                ),
                "unsupported_structure" => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Structure
                    )),
                    "{}",
                    case.name
                ),
                _ => panic!("unfrozen outcome"),
            }
            assert_eq!(image.as_bytes(), bytes);
            checked += 1;
        }
    }
    assert_eq!(checked, 36);
}

#[test]
fn before_change_fields_keep_original_intents_and_forbid_delete_context() {
    let case = &cases::CASES[0];
    let (bytes, _, _) = model::source(6, "BeforeChange", case.body);
    let image = capture(&bytes);
    for updates in [
        vec![FixedScalarUpdate::new(0, Value::Int32(9), Value::Int32(11))],
        vec![FixedScalarUpdate::new(
            100,
            Value::Int32(10),
            Value::Int32(11),
        )],
        vec![
            FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(11)),
            FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(12)),
        ],
    ] {
        assert_eq!(
            image
                .prepare_ace16_before_change_fields(
                    "Items",
                    SELECTED,
                    BeforeEventIntents::FixedUpdate {
                        column: "Score",
                        updates: &updates
                    },
                    &mut budget(),
                    &|| false,
                )
                .err(),
            Some(BeforeEventGuardError::Intent(EditError::Conflict))
        );
    }
    assert_eq!(
        image
            .prepare_ace16_before_change_fields(
                "Items",
                SELECTED,
                BeforeEventIntents::DeleteOrdinals(&[]),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Context
        ))
    );
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn before_change_fields_require_every_extra_program_even_without_rows() {
    for variant in [
        selectors::Variant::Valid,
        selectors::Variant::SecondOpaque,
        selectors::Variant::SecondMalformed,
    ] {
        let original = selectors::source(6, variant);
        let image = capture(&original.bytes);
        let one = [BeforeEventGuardSelector::new(3, "GuardA")];
        let both = [
            BeforeEventGuardSelector::new(3, "GuardA"),
            BeforeEventGuardSelector::new(3, "GuardB"),
        ];
        let empty = BeforeEventIntents::FixedUpdate {
            column: "Score",
            updates: &[],
        };
        assert_eq!(
            image
                .prepare_ace16_before_change_fields("Items", &one, empty, &mut budget(), &|| false)
                .err(),
            Some(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure
            ))
        );
        let result =
            image.prepare_ace16_before_change_fields("Items", &both, empty, &mut budget(), &|| {
                false
            });
        if matches!(variant, selectors::Variant::Valid) {
            let result = result.unwrap();
            assert_eq!(result.final_rows().unwrap().len(), 0);
            assert_eq!(result.programs().len(), 2);
            assert!(result.rejection().is_none());
        } else {
            assert_eq!(
                result.err(),
                Some(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Structure
                ))
            );
        }
        assert_eq!(image.as_bytes(), original.bytes);
    }
}

#[test]
fn before_change_fields_exact_pools_and_terminal_cancellation_publish_nothing() {
    let case = &cases::CASES[1];
    let (bytes, _, _) = model::source(6, "BeforeChange", case.body);
    let image = capture(&bytes);
    let mut measured = budget();
    let initial = [
        measured.remaining(Resource::Bytes),
        measured.remaining(Resource::Rows),
        measured.remaining(Resource::Steps),
    ];
    let polls = Cell::new(0);
    prepare(&image, case, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let used = [
        initial[0] - measured.remaining(Resource::Bytes),
        initial[1] - measured.remaining(Resource::Rows),
        initial[2] - measured.remaining(Resource::Steps),
    ];
    assert!(used.iter().all(|value| *value > 0));
    prepare(
        &image,
        case,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    for pool in 0..3 {
        let mut limits = used;
        limits[pool] -= 1;
        assert!(matches!(
            prepare(
                &image,
                case,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            )
            .err(),
            Some(BeforeEventGuardError::Inspect(InspectError::Limit(_)))
        ));
    }
    let final_poll = polls.get();
    polls.set(0);
    assert_eq!(
        prepare(&image, case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == final_poll
        })
        .err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(image.as_bytes(), bytes);
}
