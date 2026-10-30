use super::{Observed, prove};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate,
    NativeImage,
    preservation::table_extra::tests::guard_selector_sources::model::{self, Variant},
    table::mutation::guards::{compile, evaluate, source},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn before_guard_authority_rejects_correlated_selector_compilation_and_report() {
    let source = model::source(6, Variant::Valid);
    let image = NativeImage::capture(
        &mut Cursor::new(source.bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    let original_selectors = [BeforeEventGuardSelector::new(3, "GuardA")];
    let staged_selectors = [BeforeEventGuardSelector::new(3, "GuardB")];
    assert_eq!(
        image
            .prepare_ace16_before_event_guards(
                "Items",
                &original_selectors,
                original,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rejection()
            .unwrap()
            .number(),
        71
    );
    assert_eq!(
        image
            .prepare_ace16_before_event_guards(
                "Items",
                &staged_selectors,
                original,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rejection()
            .unwrap()
            .number(),
        73
    );

    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let staged = compile::derive(
        &image,
        &table,
        &staged_selectors,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let frames = source::frames(&image, &table, original, &mut budget(), &|| false).unwrap();
    let (trace, rejection) =
        evaluate::run(&staged, &frames, original.event(), &mut budget(), &|| false).unwrap();
    assert_eq!(rejection.as_ref().unwrap().number(), 73);
    assert_eq!(
        prove(
            &image,
            "Items",
            &original_selectors,
            original,
            Observed {
                compiled: &staged,
                trace: &trace,
                rejection: rejection.as_ref()
            },
            &mut budget(),
            &|| false
        )
        .err(),
        Some(BeforeEventGuardError::Preservation)
    );
}
