use super::{cases, model};
use crate::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardPreparation, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardTraceKind, BeforeGuardUnsupported, EditError, FixedScalarUpdate,
    InspectError, NativeImage, RowDelete, RowInsert,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SELECTORS: &[BeforeEventGuardSelector<'static>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 16_384, 100_000_000)
}

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
) -> Result<BeforeEventGuardPreparation<'a>, BeforeEventGuardError> {
    match case.intent {
        cases::Intent::Update {
            row,
            expected,
            value,
        } => image.prepare_ace16_before_event_guards(
            "Items",
            SELECTORS,
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[FixedScalarUpdate::new(
                    row,
                    Value::Int32(expected),
                    Value::Int32(value),
                )],
            },
            budget,
            cancelled,
        ),
        cases::Intent::Insert { id, score } => image.prepare_ace16_before_event_guards(
            "Items",
            SELECTORS,
            BeforeEventIntents::Insert(&[RowInsert::new(vec![
                Value::Int32(id),
                Value::Int32(score),
            ])]),
            budget,
            cancelled,
        ),
        cases::Intent::Delete { row } => image.prepare_ace16_before_event_guards(
            "Items",
            SELECTORS,
            BeforeEventIntents::Delete(&[RowDelete::new(row, model::original_record(row))]),
            budget,
            cancelled,
        ),
        cases::Intent::UpdateTwo => image.prepare_ace16_before_event_guards(
            "Items",
            SELECTORS,
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[
                    FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(11)),
                    FixedScalarUpdate::new(1, Value::Int32(20), Value::Int32(19)),
                ],
            },
            budget,
            cancelled,
        ),
    }
}

fn trace_names(report: &BeforeEventGuardPreparation<'_>) -> Vec<String> {
    report
        .trace()
        .iter()
        .map(|step| match step.kind() {
            BeforeGuardTraceKind::CompiledAll => "CompiledAll".into(),
            BeforeGuardTraceKind::Begin => {
                if report.event() == BeforeEvent::Insert {
                    format!("BeginInsert:{}", step.row().unwrap())
                } else {
                    format!("Begin:{}", step.row().unwrap())
                }
            }
            BeforeGuardTraceKind::Condition(value) => format!("If:{value}"),
            BeforeGuardTraceKind::End => format!("End:{}", step.row().unwrap()),
            BeforeGuardTraceKind::Raised(number) => format!("Raise:{number}"),
            BeforeGuardTraceKind::Passed => "Passed".into(),
            BeforeGuardTraceKind::Rejected => "Rejected".into(),
        })
        .collect()
}

#[test]
fn before_event_preparation_matches_all_frozen_native_source_traces() {
    for case in cases::CASES {
        for code in [5, 6] {
            let (bytes, xml, _) = model::source(code, case.event, case.body);
            let image = capture(&bytes);
            let result = prepare(&image, case, &mut budget(), &|| false);
            match case.outcome {
                cases::Outcome::Pass | cases::Outcome::Raised { .. } => {
                    let report = result
                        .unwrap_or_else(|error| panic!("{} / header {code}: {error}", case.name));
                    assert_eq!(
                        trace_names(&report),
                        case.trace,
                        "{} / header {code}",
                        case.name
                    );
                    match case.outcome {
                        cases::Outcome::Pass => assert!(report.rejection().is_none()),
                        cases::Outcome::Raised {
                            number,
                            description,
                        } => {
                            let rejection = report.rejection().unwrap();
                            assert_eq!(rejection.number(), number, "{}", case.name);
                            assert_eq!(rejection.description(), description, "{}", case.name);
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(report.table_object_id(), CatalogScope::new(7).object_id(10));
                    assert_eq!(report.source_bytes(), bytes);
                    assert_eq!(report.programs().len(), 1);
                    assert_eq!(report.programs()[0].group(), 3);
                    assert_eq!(report.programs()[0].group_ordinal(), 0);
                    assert_eq!(report.programs()[0].name(), "StoredGuard");
                    assert_eq!(report.programs()[0].property(), 0);
                    assert_eq!(report.programs()[0].catalog_reference(), (4, 2));
                    assert_eq!(
                        report.programs()[0].native_property_bytes(),
                        xml.encode_utf16()
                            .flat_map(u16::to_le_bytes)
                            .collect::<Vec<_>>()
                    );
                    for step in report.trace() {
                        if let Some(span) = step.source_span() {
                            assert_eq!(step.program(), Some(0));
                            assert!(step.node().is_some());
                            assert!(xml[span.start()..span.end()].starts_with('<'));
                        } else {
                            assert!(step.program().is_none());
                            assert!(step.node().is_none());
                        }
                    }
                }
                cases::Outcome::UnsupportedContext => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Context
                    )),
                    "{}",
                    case.name
                ),
                cases::Outcome::UnsupportedStructure => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Structure
                    )),
                    "{}",
                    case.name
                ),
                cases::Outcome::UnsupportedCondition => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::ConditionType),
                    "{}",
                    case.name
                ),
                cases::Outcome::ExpressionError => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::Expression),
                    "{}",
                    case.name
                ),
                cases::Outcome::ArgumentType => assert_eq!(
                    result.err(),
                    Some(BeforeEventGuardError::ArgumentType),
                    "{}",
                    case.name
                ),
            }
            assert_eq!(image.as_bytes(), bytes, "{} source changed", case.name);
        }
    }
}

#[test]
fn before_event_preparation_enforces_original_intents_and_complete_width() {
    let case = &cases::CASES[0];
    let (bytes, _, _) = model::source(6, case.event, case.body);
    let image = capture(&bytes);
    assert!(
        prepare(&image, case, &mut budget(), &|| false)
            .unwrap()
            .rejection()
            .is_none()
    );
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
                .prepare_ace16_before_event_guards(
                    "Items",
                    SELECTORS,
                    BeforeEventIntents::FixedUpdate {
                        column: "Score",
                        updates: &updates
                    },
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(BeforeEventGuardError::Intent(EditError::Conflict))
        );
    }
    assert_eq!(
        image
            .prepare_ace16_before_event_guards(
                "Items",
                SELECTORS,
                BeforeEventIntents::Insert(&[RowInsert::new(vec![Value::Int32(4)])]),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(BeforeEventGuardError::Intent(EditError::Unsupported(
            crate::WriteBlocker::Column
        )))
    );

    let delete_case = &cases::CASES[4];
    let (delete_bytes, _, _) = model::source(6, delete_case.event, delete_case.body);
    let delete_image = capture(&delete_bytes);
    let mut expected = model::original_record(0);
    expected[6] ^= 1;
    assert_eq!(
        delete_image
            .prepare_ace16_before_event_guards(
                "Items",
                SELECTORS,
                BeforeEventIntents::Delete(&[RowDelete::new(0, expected)]),
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(BeforeEventGuardError::Intent(EditError::Conflict))
    );
    assert_eq!(image.as_bytes(), bytes);
    assert_eq!(delete_image.as_bytes(), delete_bytes);
}

#[test]
fn before_event_preparation_resources_and_final_cancel_publish_no_report() {
    let case = &cases::CASES[0];
    let (bytes, _, _) = model::source(6, case.event, case.body);
    let image = capture(&bytes);
    for mut exhausted in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(100_000_000, 100, 0),
        ResourceBudget::new(100_000_000, 0, 1_000_000),
    ] {
        assert!(matches!(
            prepare(&image, case, &mut exhausted, &|| false).err(),
            Some(BeforeEventGuardError::Inspect(InspectError::Limit(_)))
        ));
    }
    assert_eq!(
        prepare(&image, case, &mut budget(), &|| true).err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Cancelled))
    );
    let polls = Cell::new(0_usize);
    let report = prepare(&image, case, &mut budget(), &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert!(report.rejection().is_none());
    let healthy = polls.get();
    assert!(healthy > 10);
    polls.set(0);
    assert_eq!(
        prepare(&image, case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == healthy
        })
        .err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(image.as_bytes(), bytes);
}
