use super::{SourceWitness, prepare, prove_groups, source};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, EditError,
    FixedColumnUpdates, FixedScalarUpdate, NativeImage,
    preservation::table_extra::tests::guard_sources::model,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SELECTORS: &[BeforeEventGuardSelector<'_>] =
    &[BeforeEventGuardSelector::new(3, "StoredGuard")];
const CURRENT: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]</Argument></Action></Statements>";

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn image() -> NativeImage {
    // The body is byte-exact frozen logical case4; this test invokes read-only
    // authority helpers, not a physical writer on this older source family.
    let (bytes, _, _) = model::source(6, "BeforeChange", CURRENT);
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn before_change_group_proof_rejects_another_valid_original_request_cache() {
    let image = image();
    let input = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let alternative = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(13),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &input,
    };
    let other = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &alternative,
    };
    let id = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(1))];
    let score = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(13),
    )];
    let observed = [
        FixedColumnUpdates::new("Items", "Id", &id),
        FixedColumnUpdates::new("Items", "Score", &score),
    ];
    let other_prepared =
        prepare(&image, "Items", SELECTORS, other, &mut budget(), &|| false).unwrap();
    assert_eq!(
        other_prepared.execution.rows.as_ref().unwrap()[0].values(),
        [Value::Int32(1), Value::Int32(13)]
    );
    prove_groups(
        &image,
        "Items",
        SELECTORS,
        other,
        &observed,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prove_groups(
            &image,
            "Items",
            SELECTORS,
            original,
            &observed,
            &mut budget(),
            &|| false
        )
        .err(),
        Some(BeforeEventGuardError::Preservation)
    );
}

#[test]
fn before_change_extra_witness_is_exact_source_bound_and_rechecks_program_bytes() {
    let image = image();
    let input = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let original = BeforeEventIntents::FixedUpdate {
        column: "Score",
        updates: &input,
    };
    let prepared = prepare(&image, "Items", SELECTORS, original, &mut budget(), &|| {
        false
    })
    .unwrap();
    let table = source::table(&image, "Items", &mut budget(), &|| false).unwrap();
    let mut witness = SourceWitness {
        source_address: image.as_bytes().as_ptr() as usize,
        source_length: image.as_bytes().len(),
        definition: table.schema().definition_page(),
        object: prepared.object,
        programs: prepared.compiled.programs,
    };
    witness
        .require(&image, table.schema(), &mut budget(), &|| false)
        .unwrap();
    let other = NativeImage::capture(
        &mut Cursor::new(image.as_bytes()),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(image.as_bytes(), other.as_bytes());
    assert_eq!(
        witness
            .require(&other, table.schema(), &mut budget(), &|| false)
            .err(),
        Some(EditError::Preservation)
    );
    witness.programs[0].raw[0] ^= 1;
    assert_eq!(
        witness
            .require(&image, table.schema(), &mut budget(), &|| false)
            .err(),
        Some(EditError::Preservation)
    );
}
