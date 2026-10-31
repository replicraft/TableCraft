//! Working selector-free logical compilation over the source-only fixture owner.
use super::{Case, SCOPE, budget, source};
use crate::preservation::table_extra::tests::guard_selector_sources::model as duplicate;
use crate::{
    BeforeEventGuardError, BeforeEventIntents, BeforeGuardUnsupported, FixedScalarUpdate,
    NativeImage,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[test]
fn native_owned_alias_dispatches_sequential_current_and_frozen_old_without_selectors() {
    for code in [5, 6] {
        let bytes = source(code, Case::Sequential);
        let image = NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
            .unwrap();
        let updates = [FixedScalarUpdate::new(
            0,
            Value::Int32(10),
            Value::Int32(17),
        )];
        let prepared = image
            .prepare_native_before_change_fields(
                SCOPE.object_id(10),
                BeforeEventIntents::FixedUpdate {
                    column: "Score",
                    updates: &updates,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let rows = prepared.final_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ordinal(), 0);
        assert_eq!(rows[0].values(), [Value::Int32(1), Value::Int32(28)]);
        assert_eq!(prepared.assignments().len(), 2);
        assert_eq!(prepared.programs().len(), 1);
        assert_eq!(prepared.programs()[0].name(), "StoredGuard");
        assert_eq!(prepared.programs()[0].catalog_reference(), (4, 2));
        assert_eq!(prepared.source_bytes(), image.as_bytes());
    }
}

#[test]
fn names_only_and_required_after_events_refuse_before_row_intent_validation() {
    for code in [5, 6] {
        for (case, cause) in [
            (Case::NamesOnly, BeforeGuardUnsupported::NativeCoverage),
            (Case::RequiredAfter, BeforeGuardUnsupported::NativeEvent),
        ] {
            let bytes = source(code, case);
            let image =
                NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            // This deliberately stale intent would fail if frame validation ran.
            let updates = [FixedScalarUpdate::new(
                0,
                Value::Int32(999),
                Value::Int32(17),
            )];
            assert!(matches!(
                image.prepare_native_before_change_fields(
                    SCOPE.object_id(10),
                    BeforeEventIntents::FixedUpdate {
                        column: "Score",
                        updates: &updates,
                    },
                    &mut budget(),
                    &|| false,
                ),
                Err(BeforeEventGuardError::Unsupported(actual)) if actual == cause
            ));
        }
    }
}

#[test]
fn source_derived_unsupported_program_compiles_and_refuses_even_with_no_rows() {
    let bytes = source(6, Case::UnsupportedBody);
    let image =
        NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        image.prepare_native_before_change_fields(
            SCOPE.object_id(10),
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[]
            },
            &mut budget(),
            &|| false
        ),
        Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::Structure
        ))
    ));
}

#[test]
fn native_binding_rejects_duplicate_events_and_obeys_cancellation_and_budget() {
    let bytes = duplicate::source(6, duplicate::Variant::Valid).bytes;
    let image =
        NativeImage::capture(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let intent = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &[],
    };
    assert!(matches!(
        image.prepare_native_before_change_fields(
            SCOPE.object_id(10),
            intent,
            &mut budget(),
            &|| false
        ),
        Err(BeforeEventGuardError::Unsupported(
            BeforeGuardUnsupported::NativeEventConflict
        ))
    ));
    assert!(matches!(
        image.prepare_native_before_change_fields(
            SCOPE.object_id(10),
            intent,
            &mut budget(),
            &|| true
        ),
        Err(BeforeEventGuardError::Inspect(
            crate::InspectError::Cancelled
        ))
    ));
    assert!(matches!(
        image.prepare_native_before_change_fields(
            SCOPE.object_id(10),
            intent,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(BeforeEventGuardError::Inspect(_))
    ));
}
