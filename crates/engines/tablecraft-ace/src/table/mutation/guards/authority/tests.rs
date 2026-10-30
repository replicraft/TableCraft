use super::{Observed, prove};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate,
    NativeImage,
    preservation::table_extra::tests::guard_sources::{cases, model},
    table::mutation::guards::{compile, evaluate, source},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_sql::select::Expression;

const SELECTORS: &[BeforeEventGuardSelector<'static>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(case: &cases::Case) -> NativeImage {
    let (bytes, _, _) = model::source(6, case.event, case.body);
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn before_guard_authority_rejects_valid_alternative_intents_as_original_authority() {
    let image = image(&cases::CASES[0]);
    let first = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let alternative = [FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(9))];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &first,
    };
    let other = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &alternative,
    };

    assert!(
        image
            .prepare_ace16_before_event_guards("Items", SELECTORS, original, &mut budget(), &|| {
                false
            })
            .unwrap()
            .rejection()
            .is_none()
    );
    assert_eq!(
        image
            .prepare_ace16_before_event_guards("Items", SELECTORS, other, &mut budget(), &|| false)
            .unwrap()
            .rejection()
            .unwrap()
            .number(),
        17
    );
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let compiled = compile::derive(
        &image,
        &table,
        SELECTORS,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let frames = source::frames(&image, &table, other, &mut budget(), &|| false).unwrap();
    let (trace, rejection) =
        evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
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

#[test]
fn before_guard_authority_rejects_correlated_program_cache_revision_and_result() {
    let image = image(&cases::CASES[1]);
    let updates = [FixedScalarUpdate::new(0, Value::Int32(10), Value::Int32(9))];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    let healthy = image
        .prepare_ace16_before_event_guards("Items", SELECTORS, original, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(healthy.rejection().unwrap().number(), 17);
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut compiled = compile::derive(
        &image,
        &table,
        SELECTORS,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let frames = source::frames(&image, &table, original, &mut budget(), &|| false).unwrap();
    for instruction in &mut compiled.instructions {
        if let compile::Operation::Raise { number, .. } = &mut instruction.operation {
            *number = Expression::Integer(19);
        }
    }
    let changed = String::from_utf16(
        &compiled.programs[0]
            .raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect::<Vec<_>>(),
    )
    .unwrap()
    .replace(
        "<Argument Name=\"Number\">17</Argument>",
        "<Argument Name=\"Number\">19</Argument>",
    );
    compiled.programs[0].raw = changed.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let (trace, rejection) =
        evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(rejection.as_ref().unwrap().number(), 19);
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
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

#[test]
fn before_guard_authority_rejects_wrong_condition_cache_even_with_original_revision() {
    let image = image(&cases::CASES[0]);
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    assert!(
        image
            .prepare_ace16_before_event_guards("Items", SELECTORS, original, &mut budget(), &|| {
                false
            })
            .unwrap()
            .rejection()
            .is_none()
    );
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut compiled = compile::derive(
        &image,
        &table,
        SELECTORS,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let frames = source::frames(&image, &table, original, &mut budget(), &|| false).unwrap();
    for instruction in &mut compiled.instructions {
        if let compile::Operation::If { expression, .. } = &mut instruction.operation {
            *expression = Expression::Boolean(true);
        }
    }
    let (trace, rejection) =
        evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(rejection.as_ref().unwrap().number(), 17);
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
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
