//! Description remains metadata when selecting an actual Increment source.
use super::{Mode, inspect};
#[cfg(feature = "ace16-create")]
use crate::{
    CreateColumn, InsertFieldValue, OmittedRowInsert, create_ace16_database,
    inspect_table_properties, read_table,
};
use crate::{EditError, InspectError, NativeImage, inspect_table_schema};
use std::{io::Cursor, slice};
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "ace16-create")]
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
#[cfg(feature = "ace16-create")]
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_normal_autonumber, field_presentation, wide_properties,
};

const SCOPE: CatalogScope = CatalogScope::new(498);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn mode(bytes: &[u8]) -> Result<Mode, EditError> {
    let image = capture(bytes);
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })?;
    inspect(&image, &schema, &mut budget(), &|| false)
}

fn group(name: &str, properties: &[Vec<u8>]) -> Vec<u8> {
    let name = wide_properties::name(name);
    let mut body = ((name.len() + 4) as u32).to_le_bytes().to_vec();
    body.extend(name);
    for property in properties {
        body.extend(property);
    }
    wide_properties::block(1, &body)
}

fn source(groups: &[Vec<u8>], keys: &[&str]) -> Vec<u8> {
    let mut blocks = vec![wide_properties::names(keys)];
    blocks.extend_from_slice(groups);
    let mut bytes = field_presentation::database(&blocks, false);
    // Original independent definition: Id becomes a native LONG Increment field.
    // Mode inspection deliberately does not perform insertion or counter proof.
    bytes[10 * 4096 + 63 + 15] = 7;
    bytes
}

fn description(flags: u8, native_type: u8, text: &str) -> Vec<u8> {
    field_presentation::property(0, flags, native_type, &wide_properties::text(text))
}

#[test]
fn bounded_inert_description_selects_increment_on_primary_or_companion() {
    for name in ["Id", "Score"] {
        for text in ["", "  Now() 日本語 😀  "] {
            let bytes = source(
                &[group(name, &[description(1, 10, text)])],
                &["Description"],
            );
            assert!(matches!(mode(&bytes), Ok(Mode::Increment)));
        }
    }
}

#[test]
fn description_selection_keeps_unknown_flags_carriers_duplicates_and_limits_closed() {
    for name in ["Id", "Score"] {
        for properties in [
            vec![description(0, 10, "inert")],
            vec![description(3, 10, "inert")],
            vec![description(1, 12, "inert")],
            vec![description(1, 9, "inert")],
            vec![description(1, 10, &"x".repeat(256))],
            vec![description(1, 10, &"😀".repeat(128))],
        ] {
            let bytes = source(&[group(name, &properties)], &["Description"]);
            assert!(matches!(mode(&bytes), Ok(Mode::Unknown)));
        }
        let duplicate = description(1, 10, "inert");
        let bytes = source(
            &[group(name, &[duplicate.clone(), duplicate])],
            &["Description"],
        );
        assert!(matches!(
            mode(&bytes),
            Err(EditError::Inspect(InspectError::Malformed))
        ));
    }
    let duplicate_group = group("Score", &[description(1, 10, "inert")]);
    let bytes = source(
        &[duplicate_group.clone(), duplicate_group],
        &["Description"],
    );
    assert!(matches!(
        mode(&bytes),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
    let bytes = source(
        &[group("Missing", &[description(1, 10, "inert")])],
        &["Description"],
    );
    assert!(matches!(mode(&bytes), Ok(Mode::Unknown)));
    for policy in [
        "Required",
        "AllowZeroLength",
        "DefaultValue",
        "Unrecognized",
    ] {
        let bytes = source(
            &[group(
                "Id",
                &[
                    description(1, 10, "inert"),
                    field_presentation::property(1, 1, 1, &[0]),
                ],
            )],
            &["Description", policy],
        );
        assert!(matches!(mode(&bytes), Ok(Mode::Unknown)));
    }
}

#[cfg(feature = "ace16-create")]
#[test]
fn builder_descriptions_preserve_omitted_default_and_first_increment_insert() {
    let default = Value::Text("Guest".into());
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryKey")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap()
            .with_description("  primary 😀  ")
            .unwrap(),
        CreateColumn::short_text("Label", 30)
            .unwrap()
            .with_required(false)
            .with_allow_zero_length(true)
            .unwrap()
            .with_default_literal(&default)
            .unwrap()
            .with_description("Now() remains inert")
            .unwrap(),
    ];
    let created = create_ace16_database("Items", &columns, &[], &mut budget(), &|| false).unwrap();
    let before = inspect_table_properties(
        &mut capture(created.as_bytes()).reader(),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let request = OmittedRowInsert::new(vec![InsertFieldValue::Omitted, InsertFieldValue::Omitted]);
    let plan = capture(created.as_bytes())
        .plan_ordinary_omitted_row_inserts("Items", &[request], &mut budget(), &|| false)
        .unwrap();
    let after = capture(plan.after_bytes());
    let rows = read_table(&mut after.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(
        rows.rows(),
        &[vec![Value::Int32(1), Value::Text("Guest".into())]]
    );
    let properties =
        inspect_table_properties(&mut after.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(properties.raw_bytes(), before.raw_bytes());
}

#[test]
fn empty_generated_groups_and_random_defaults_with_descriptions_stay_out_of_increment() {
    let empty = source(&[group("Id", &[])], &["Description"]);
    assert!(matches!(
        mode(&empty),
        Err(EditError::Inspect(InspectError::Unsupported))
    ));
    let random = field_presentation::property(1, 1, 12, &wide_properties::text("GenUniqueID()"));
    let described_random = source(
        &[
            group("Id", slice::from_ref(&random)),
            group("Score", &[description(1, 10, "inert")]),
        ],
        &["Description", "DefaultValue"],
    );
    assert!(matches!(mode(&described_random), Ok(Mode::Unknown)));
    let described_generator = source(
        &[group("Id", &[random, description(1, 10, "inert")])],
        &["Description", "DefaultValue"],
    );
    assert!(matches!(mode(&described_generator), Ok(Mode::Unknown)));
    let random_only =
        canonical_normal_autonumber::source(canonical_normal_autonumber::Case::RandomDefault);
    assert!(matches!(mode(&random_only.bytes), Ok(Mode::Random)));
}
