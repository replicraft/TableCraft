use super::model::{self, Variant};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, BeforeGuardSelectionError,
    BeforeGuardTraceKind, BeforeGuardUnsupported, FixedScalarUpdate, InspectError, NativeImage,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const ALPHA: BeforeEventGuardSelector<'static> = BeforeEventGuardSelector::new(3, "GuardA");
const BETA: BeforeEventGuardSelector<'static> = BeforeEventGuardSelector::new(3, "GuardB");

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 16_384, 100_000_000)
}

fn image(code: u8, variant: Variant) -> NativeImage {
    let source = model::source(code, variant);
    NativeImage::capture(
        &mut Cursor::new(source.bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn before_guard_selector_scope_and_caller_order_match_independent_two_property_witness() {
    for code in [5, 6] {
        let image = image(code, Variant::Valid);
        let updates = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(11),
        )];
        let original = BeforeEventIntents::FixedUpdate {
            column: "Score",
            updates: &updates,
        };
        for (selectors, expected_names, number, description, first_property) in [
            ([ALPHA, BETA], ["GuardA", "GuardB"], 71, "alpha", 0),
            ([BETA, ALPHA], ["GuardB", "GuardA"], 73, "beta", 1),
        ] {
            let report = image
                .prepare_ace16_before_event_guards(
                    "Items",
                    &selectors,
                    original,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                report
                    .programs()
                    .iter()
                    .map(|program| program.name())
                    .collect::<Vec<_>>(),
                expected_names
            );
            assert_eq!(report.programs()[0].group(), 3);
            assert_eq!(report.programs()[0].group_ordinal(), 0);
            assert_eq!(report.programs()[0].property(), first_property);
            assert_eq!(report.rejection().unwrap().number(), number);
            assert_eq!(report.rejection().unwrap().description(), Some(description));
            assert_eq!(
                report
                    .trace()
                    .iter()
                    .map(|step| step.kind())
                    .collect::<Vec<_>>(),
                [
                    BeforeGuardTraceKind::CompiledAll,
                    BeforeGuardTraceKind::Begin,
                    BeforeGuardTraceKind::Raised(number),
                    BeforeGuardTraceKind::Rejected
                ]
            );
            assert_eq!(report.trace()[2].program(), Some(0));
            assert_eq!(report.trace()[1].row(), Some(0));
        }
    }
}

#[test]
fn before_guard_selector_input_errors_and_duplicate_kind_source_fail_closed() {
    let image = image(6, Variant::Valid);
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    for (selectors, error) in [
        (vec![], BeforeGuardSelectionError::Empty),
        (
            vec![BeforeEventGuardSelector::new(3, "Missing")],
            BeforeGuardSelectionError::Missing,
        ),
        (
            vec![BeforeEventGuardSelector::new(4, "GuardA")],
            BeforeGuardSelectionError::Missing,
        ),
        (
            vec![BeforeEventGuardSelector::new(3, "guarda")],
            BeforeGuardSelectionError::Missing,
        ),
        (vec![ALPHA, ALPHA], BeforeGuardSelectionError::Duplicate),
    ] {
        assert_eq!(
            image
                .prepare_ace16_before_event_guards(
                    "Items",
                    &selectors,
                    original,
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(BeforeEventGuardError::Selection(error))
        );
    }
    let duplicate_kind = self::image(6, Variant::DuplicateKind);
    assert_eq!(
        duplicate_kind
            .prepare_ace16_before_event_guards("Items", &[ALPHA], original, &mut budget(), &|| {
                false
            })
            .err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Malformed))
    );
}

#[test]
fn before_guard_selectors_compile_all_selected_bodies_and_never_run_unselected_xml() {
    for code in [5, 6] {
        for variant in [Variant::SecondOpaque, Variant::SecondMalformed] {
            let image = image(code, variant);
            let updates = [FixedScalarUpdate::new(
                0,
                Value::Int32(10),
                Value::Int32(11),
            )];
            let original = BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &updates,
            };
            let report = image
                .prepare_ace16_before_event_guards(
                    "Items",
                    &[ALPHA],
                    original,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(report.programs().len(), 1);
            assert_eq!(report.programs()[0].name(), "GuardA");
            assert_eq!(report.rejection().unwrap().number(), 71);
            let error = image
                .prepare_ace16_before_event_guards(
                    "Items",
                    &[ALPHA, BETA],
                    original,
                    &mut budget(),
                    &|| false,
                )
                .err();
            match variant {
                Variant::SecondOpaque => assert_eq!(
                    error,
                    Some(BeforeEventGuardError::Unsupported(
                        BeforeGuardUnsupported::Structure
                    ))
                ),
                Variant::SecondMalformed => assert_eq!(
                    error,
                    Some(BeforeEventGuardError::Inspect(InspectError::Malformed))
                ),
                _ => unreachable!(),
            }
        }
    }
}
