use super::super::{authority, prepare};
use super::{budget, model};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate,
    NativeImage,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
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

#[test]
fn dormant_registry_revision_is_authority_even_when_no_delete_executes() {
    let case = model::source(6, model::Recipe::DormantDeleteRaise);
    let image = capture(&case.images.before);
    let selectors = [
        BeforeEventGuardSelector::new(3, "Change"),
        BeforeEventGuardSelector::new(3, "Delete"),
    ];
    let values = [FixedScalarUpdate::new(300, Value::Null, Value::Int32(99))];
    let intent = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &values,
    };
    let mut prepared = prepare(&image, "Items", &selectors, intent, &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(prepared.compiled.programs.len(), 2);
    prepared.compiled.programs[1].raw[0] ^= 1;

    assert_eq!(
        authority::prove(
            &image,
            "Items",
            &selectors,
            intent,
            authority::Observed {
                compiled: &prepared.compiled,
                execution: &prepared.execution
            },
            &mut budget(),
            &|| false,
        ),
        Err(BeforeEventGuardError::Preservation),
    );
}

#[test]
fn correlated_joint_result_from_another_request_cannot_rebind_dispatch() {
    let case = model::source(6, model::Recipe::SequentialChange);
    let image = capture(&case.images.before);
    let selectors = [
        BeforeEventGuardSelector::new(3, "Change"),
        BeforeEventGuardSelector::new(3, "Delete"),
        BeforeEventGuardSelector::new(3, "ChangeLast"),
    ];
    let actual = [FixedScalarUpdate::new(300, Value::Null, Value::Int32(99))];
    let alternative = [FixedScalarUpdate::new(300, Value::Null, Value::Int32(98))];
    let prepared = prepare(
        &image,
        "Items",
        &selectors,
        BeforeEventIntents::FixedUpdate {
            column: "Score",
            updates: &alternative,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepared.execution.rows.as_ref().unwrap()[0].values(),
        &[Value::Int32(301), Value::Int32(98), Value::Int32(301)]
    );

    assert_eq!(
        authority::prove(
            &image,
            "Items",
            &selectors,
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &actual
            },
            authority::Observed {
                compiled: &prepared.compiled,
                execution: &prepared.execution
            },
            &mut budget(),
            &|| false,
        ),
        Err(BeforeEventGuardError::Preservation),
    );
}
