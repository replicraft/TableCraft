//! Working explicit logical consumers of the source-bound observation report.
use super::{budget, capture, guard_selector_sources, model, observed};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, BeforeGuardUnsupported,
    FixedScalarUpdate, InspectError,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const ROOT: u64 = 10;
const SELECTED: [BeforeEventGuardSelector<'static>; 1] =
    [BeforeEventGuardSelector::new(3, "ValidateLedger")];

#[test]
fn bound_guard_preparation_retains_original_image_after_observation_drop() {
    for code in [5, 6] {
        let source = observed::source(code, observed::Case::NonstandardEventProperty);
        let scope = CatalogScope::new(19);
        let image = capture(&source.bytes, scope);
        let updates = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(11),
        )];
        let intent = BeforeEventIntents::FixedUpdate {
            column: "Score",
            updates: &updates,
        };
        let prepared = {
            let coverage = image
                .inspect_table_macro_coverage(scope.object_id(ROOT), &mut budget(), &|| false)
                .unwrap();
            assert!(std::ptr::eq(coverage.image(), &image));
            coverage
                .prepare_before_event_guards(&SELECTED, intent, &mut budget(), &|| false)
                .unwrap()
        };
        let direct = image
            .prepare_ace16_before_event_guards("Items", &SELECTED, intent, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.source_bytes(), image.as_bytes());
        assert_eq!(prepared.table_object_id(), scope.object_id(ROOT));
        assert!(prepared.rejection().is_none());
        assert_eq!(prepared.trace(), direct.trace());
        assert!(prepared.programs() == direct.programs());
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

#[test]
fn bound_guards_require_complete_properties_without_changing_legacy_subset() {
    for code in [5, 6] {
        let source = guard_selector_sources::model::source(
            code,
            guard_selector_sources::model::Variant::Valid,
        );
        let scope = CatalogScope::new(19);
        let image = capture(&source.bytes, scope);
        let selection = [BeforeEventGuardSelector::new(3, "GuardA")];
        let original = BeforeEventIntents::FixedUpdate {
            column: "Score",
            updates: &[],
        };
        let direct = image
            .prepare_ace16_before_event_guards(
                "Items",
                &selection,
                original,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(direct.programs().len(), 1);
        let coverage = image
            .inspect_table_macro_coverage(scope.object_id(ROOT), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            coverage
                .prepare_before_event_guards(&selection, original, &mut budget(), &|| false)
                .err(),
            Some(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure
            ))
        );
        assert_eq!(
            coverage
                .prepare_before_change_fields(&selection, original, &mut budget(), &|| false)
                .err(),
            Some(BeforeEventGuardError::Unsupported(
                BeforeGuardUnsupported::Structure
            ))
        );
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

#[test]
fn bound_preparation_retains_unresolved_carriers_as_refusals_even_empty() {
    for code in [5, 6] {
        for case in model::CASES {
            let source = model::source(code, case);
            let scope = CatalogScope::new(19);
            let image = capture(&source.bytes, scope);
            let coverage = image
                .inspect_table_macro_coverage(scope.object_id(source.id), &mut budget(), &|| false)
                .unwrap();
            let original = BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[],
            };
            let expected =
                BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::SourceProfile);
            assert_eq!(
                coverage
                    .prepare_before_event_guards(&SELECTED, original, &mut budget(), &|| false)
                    .err(),
                Some(expected)
            );
            assert_eq!(
                coverage
                    .prepare_before_change_fields(&SELECTED, original, &mut budget(), &|| false)
                    .err(),
                Some(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::SourceProfile
                ))
            );
            assert_eq!(image.as_bytes(), source.bytes);
        }
        for case in [
            observed::Case::NamesOnly,
            observed::Case::OpaqueExtra,
            observed::Case::OpaqueGroup,
            observed::Case::MalformedXml,
            observed::Case::NonTextProperty,
        ] {
            let source = observed::source(code, case);
            let scope = CatalogScope::new(19);
            let image = capture(&source.bytes, scope);
            let coverage = image
                .inspect_table_macro_coverage(scope.object_id(ROOT), &mut budget(), &|| false)
                .unwrap();
            let original = BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[],
            };
            assert_eq!(
                coverage
                    .prepare_before_event_guards(&SELECTED, original, &mut budget(), &|| false)
                    .err(),
                Some(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Structure
                ))
            );
            assert_eq!(
                coverage
                    .prepare_before_change_fields(&SELECTED, original, &mut budget(), &|| false)
                    .err(),
                Some(BeforeEventGuardError::Unsupported(
                    BeforeGuardUnsupported::Structure
                ))
            );
            assert_eq!(image.as_bytes(), source.bytes);
        }
    }
}

#[test]
fn bound_preparation_preflight_keeps_cancellation_and_work_limits_typed() {
    let source = observed::source(6, observed::Case::NonstandardEventProperty);
    let scope = CatalogScope::new(19);
    let image = capture(&source.bytes, scope);
    let coverage = image
        .inspect_table_macro_coverage(scope.object_id(ROOT), &mut budget(), &|| false)
        .unwrap();
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &[],
    };
    assert_eq!(
        coverage
            .prepare_before_event_guards(&SELECTED, original, &mut budget(), &|| true)
            .err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(
        coverage
            .prepare_before_change_fields(&SELECTED, original, &mut budget(), &|| true)
            .err(),
        Some(BeforeEventGuardError::Inspect(InspectError::Cancelled))
    );
    for fields in [false, true] {
        let mut limited = ResourceBudget::new(u64::MAX, u64::MAX, 0);
        let error = if fields {
            coverage
                .prepare_before_change_fields(&SELECTED, original, &mut limited, &|| false)
                .err()
        } else {
            coverage
                .prepare_before_event_guards(&SELECTED, original, &mut limited, &|| false)
                .err()
        };
        assert!(matches!(
            error,
            Some(BeforeEventGuardError::Inspect(InspectError::Limit(_)))
        ));
    }
    assert_eq!(image.as_bytes(), source.bytes);
}
