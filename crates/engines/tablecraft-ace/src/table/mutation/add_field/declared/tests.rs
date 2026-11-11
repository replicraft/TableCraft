//! Independent complete Currency images retain absence and sparse native identities.
use super::*;
use crate::{CreateColumn, inspect_table_properties, inspect_table_schema};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields as raw, field_changes, fields, populated,
};

const SCOPE: CatalogScope = CatalogScope::new(30431);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 500_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}
fn declaration(name: &str) -> DeclaredFieldAdditionDefinition {
    DeclaredFieldAdditionDefinition::capture(
        CreateColumn::new(name, ScalarType::Currency),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn declared_currency_changes_only_keyless_tdef_and_keeps_absent_properties_and_old_null() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root =
        u16::from_le_bytes(seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap()) as usize;
    let full = field_changes::keyless_first_field(&seed, "Items");
    let sparse = field_changes::drop_field(&full, definition, catalog_root, "Id");
    let absent = populated::numbers(&seed, "Bare", &[Some(7), None]);
    let unicode_table = field_changes::keyless_first_field(&seed, "項目");
    let unused_dictionary = raw::with_unused_dictionary_name(&sparse, catalog_root, "Amount");
    for (before, table, active) in [
        (&full, "Items", 2),
        (&sparse, "Items", 1),
        (&absent, "Bare", 1),
        (&unicode_table, "項目", 2),
        (&unused_dictionary, "Items", 1),
    ] {
        let expected = raw::after(before, definition, "Amount");
        let candidate = image(before)
            .prepare_retained_declared_field_addition(
                table,
                declaration("Amount"),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(candidate.before_bytes(), before);
        same(candidate.after_bytes(), &expected);
        assert_eq!(candidate.native_table_id(), definition as i32);
        assert_eq!(candidate.definition_page(), definition);
        assert_eq!(candidate.column_count(), active + 1);
        assert_eq!(
            candidate.native_number(),
            if table == "Bare" { 1 } else { 2 }
        );
        let properties = inspect_table_properties(
            &mut Cursor::new(&expected),
            SCOPE,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            properties
                .groups()
                .iter()
                .all(|group| !group.name().eq_ignore_ascii_case("Amount"))
        );
        let schema = inspect_table_schema(
            &mut Cursor::new(&expected),
            SCOPE,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let field = schema.columns().last().unwrap();
        assert_eq!(field.native_type(), 5);
        assert_eq!(field.storage_length(), 8);
        let rows = crate::read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            rows.rows()
                .iter()
                .all(|row| row.last() == Some(&Value::Null))
        );
    }
}

#[test]
fn declared_request_rejects_gallery_policies_and_candidate_keeps_source_guards() {
    let zero = Value::Currency(tablecraft_core::values::Currency::from_scaled(0));
    for column in [
        CreateColumn::new("Amount", ScalarType::Int32),
        CreateColumn::new("Amount", ScalarType::Currency).with_required(false),
        CreateColumn::new("Amount", ScalarType::Currency)
            .with_default_literal(&zero)
            .unwrap(),
    ] {
        assert!(
            DeclaredFieldAdditionDefinition::capture(column, &mut budget(), &|| false).is_err()
        );
    }
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    assert!(matches!(
        image(&seed).prepare_retained_declared_field_addition(
            "Table1",
            declaration("Amount"),
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
    let keyed_properties = field_changes::keyless_first_field(&seed, "Items");
    assert!(matches!(
        image(&keyed_properties).prepare_retained_declared_field_addition(
            "Items",
            declaration("Id"),
            &mut budget(),
            &|| false
        ),
        Err(Error::DuplicateName)
    ));
    let before = populated::numbers(&seed, "Bare", &[Some(7)]);
    assert!(matches!(
        image(&before).prepare_retained_declared_field_addition(
            "Bare",
            declaration("CopiedScore"),
            &mut budget(),
            &|| false
        ),
        Err(Error::DuplicateName)
    ));
    assert!(
        image(&before)
            .prepare_retained_declared_field_addition(
                "Bare",
                declaration("Amount"),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
}

#[test]
fn declared_proof_rejects_unrelated_bytes_and_target_descriptor_changes() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let before = populated::numbers(&seed, "Bare", &[Some(7)]);
    let source = Family::Retained
        .capture(image(&before), "Bare", &mut budget(), &|| false)
        .unwrap();
    let declaration = declaration("Amount");
    let plan = Plan::for_field(
        source.image(),
        &source.table().schema,
        declaration.field(0).unwrap(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = raw::after(&before, definition, "Amount");
    for offset in [
        3 * 4096 + 3000,
        (definition as usize + 1) * 4096 + 4000,
        definition as usize * 4096 + 63 + 25 + 23,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        assert!(matches!(
            proof::candidate(
                &source,
                &plan,
                &declaration,
                &forged,
                &mut budget(),
                &|| false
            ),
            Err(Error::Preservation)
        ));
    }
}

#[test]
fn unqualified_unicode_field_namespace_refuses_without_restricting_table_or_payload_names() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = seed.len() / 4096;
    let mut before = field_changes::keyless_first_field(&seed, "Items");
    let name = definition * 4096 + 121;
    assert_eq!(
        &before[name..name + 10],
        &[b'S', 0, b'c', 0, b'o', 0, b'r', 0, b'e', 0]
    );
    // Same UTF16 length: no descriptor, property or row geometry is changed.
    // This is an unqualified namespace, not a claimed Access collision result.
    before[name..name + 2].copy_from_slice(&0x017f_u16.to_le_bytes());
    let observed = Family::Retained
        .observe(&image(&before), "Items", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(observed.schema.columns()[1].name(), "ſcore");
    assert!(matches!(
        image(&before).prepare_retained_declared_field_addition(
            "Items",
            declaration("Score"),
            &mut budget(),
            &|| false
        ),
        Err(Error::Source)
    ));
}

#[test]
fn inactive_column_property_groups_cannot_be_resurrected_as_unadorned_policies() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let catalog_root =
        u16::from_le_bytes(seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap()) as usize;
    let full = field_changes::keyless_first_field(&seed, "Items");
    let sparse = field_changes::drop_field(&full, definition, catalog_root, "Id");
    let before = raw::with_retired_required(&sparse, catalog_root, "Amount");
    let properties = inspect_table_properties(
        &mut Cursor::new(&before),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "Amount")
        .unwrap();
    assert_eq!(group.properties().len(), 1);
    assert!(
        group.properties()[0]
            .name()
            .eq_ignore_ascii_case("Required")
    );
    // Existing NoRequired admission may reject the inactive group upstream.
    // Either way it cannot acquire a new field identity through this candidate.
    assert!(
        image(&before)
            .prepare_retained_declared_field_addition(
                "Items",
                declaration("Amount"),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}
