use super::{Observed, prove};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, FixedScalarUpdate,
    NativeImage,
    preservation::table_extra::tests::guard_sources::model,
    table::mutation::guards::{compile, fields::evaluate, source},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_sql::select::Expression;

const SELECTORS: &[BeforeEventGuardSelector<'_>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];
const CONSTANT: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">12</Argument></Action></Statements>";
const OLD: &str = r##"<Statements><Action Name="SetField"><Argument Name="Field">Sc&#111;re</Argument><Argument Name="Value"><![CDATA[[Old].[Score]]]></Argument></Action></Statements>"##;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(body: &str) -> NativeImage {
    let (bytes, _, _) = model::source(6, "BeforeChange", body);
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn before_change_authority_rejects_correlated_instruction_and_final_row() {
    let image = image(CONSTANT);
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let frames = source::frames(&image, &table, original, &mut budget(), &|| false).unwrap();
    let mut compiled = compile::derive_fields(
        &image,
        &table,
        SELECTORS,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let healthy = evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
        false
    })
    .unwrap();
    prove(
        &image,
        "Items",
        SELECTORS,
        original,
        Observed {
            compiled: &compiled,
            execution: &healthy,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();

    let compile::Operation::SetField { expression, .. } = &mut compiled.instructions[0].operation
    else {
        panic!("literal SetField missing")
    };
    *expression = Expression::Integer(13);
    let forged = evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        forged.rows.as_ref().unwrap()[0].values(),
        [Value::Int32(1), Value::Int32(13)]
    );
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
                execution: &forged
            },
            &mut budget(),
            &|| false
        )
        .err(),
        Some(BeforeEventGuardError::Preservation)
    );
}

#[test]
fn before_change_authority_rejects_forged_frozen_old_and_program_witness() {
    // Reuse the already-qualified Old program from the frozen literal matrix.
    let image = image(OLD);
    let updates = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &updates,
    };
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut frames = source::frames(&image, &table, original, &mut budget(), &|| false).unwrap();
    let mut compiled = compile::derive_fields(
        &image,
        &table,
        SELECTORS,
        original.event(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let healthy = evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
        false
    })
    .unwrap();
    prove(
        &image,
        "Items",
        SELECTORS,
        original,
        Observed {
            compiled: &compiled,
            execution: &healthy,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();

    frames[0].original.as_mut().unwrap()[1] = Value::Int32(99);
    let forged = evaluate::run(&compiled, &frames, original.event(), &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        forged.rows.as_ref().unwrap()[0].values(),
        [Value::Int32(1), Value::Int32(99)]
    );
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
                execution: &forged
            },
            &mut budget(),
            &|| false
        )
        .err(),
        Some(BeforeEventGuardError::Preservation)
    );

    // Corrupt only the retained source witness; unchanged correct final rows and
    // traces still cannot authorize another program against the original source.
    compiled.programs[0].raw[0] ^= 1;
    assert_eq!(
        prove(
            &image,
            "Items",
            SELECTORS,
            original,
            Observed {
                compiled: &compiled,
                execution: &healthy
            },
            &mut budget(),
            &|| false
        )
        .err(),
        Some(BeforeEventGuardError::Preservation)
    );
}
