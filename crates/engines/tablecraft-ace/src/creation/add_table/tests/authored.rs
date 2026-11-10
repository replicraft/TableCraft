//! Actual source catalog gaps select native property placement before planning.
use crate::{
    AddTableSourceProfile, CreateColumn, FirstTableDefinition, NativeImage, PropertyGroupKind,
    add_table_source_capabilities, authored_add_table_source_capabilities,
    inspect_table_properties, read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{self as raw, fields};

const SCOPE: CatalogScope = CatalogScope::new(7361);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 10_000_000, 512 * 1024 * 1024)
}

fn word(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize
}

fn catalog_gap(bytes: &[u8]) -> usize {
    let page = &bytes[4 * 4096..5 * 4096];
    let count = word(page, 12);
    let start = (0..count)
        .map(|slot| word(page, 14 + 2 * slot) & 0x1fff)
        .min()
        .unwrap();
    start - (14 + count * 2)
}

fn before() -> Vec<u8> {
    let mut bytes = fields::source(fields::Case::FirstNumber).bytes;
    let mut ordinal = 0;
    while catalog_gap(&bytes) > 400 {
        bytes = raw::append(&bytes, &format!("T{ordinal}"), raw::Target::GeneratedNumber);
        ordinal += 1;
    }
    // The independent source has room for the named external descriptor and
    // directory cell, while even the target's Description payload cannot fit.
    assert!((150..=400).contains(&catalog_gap(&bytes)));
    bytes
}

fn name(text: &str) -> Vec<u8> {
    let wide = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut bytes = (wide.len() as u16).to_le_bytes().to_vec();
    bytes.extend(wide);
    bytes
}

fn block(kind: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((6 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn properties(description: &str) -> Vec<u8> {
    let mut dictionary = name("Required");
    dictionary.extend(name("DefaultValue"));
    dictionary.extend(name("Description"));
    let label = name("Score");
    let mut group = ((4 + label.len()) as u32).to_le_bytes().to_vec();
    group.extend(label);
    group.extend([9, 0, 1, 1, 0, 0, 1, 0, 0]);
    group.extend([10, 0, 1, 12, 1, 0, 2, 0, b'0', 0]);
    let text = description
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    group.extend(((8 + text.len()) as u16).to_le_bytes());
    group.extend([1, 10, 2, 0]);
    group.extend((text.len() as u16).to_le_bytes());
    group.extend(text);
    let mut bytes = b"MR2\0".to_vec();
    bytes.extend(block(128, &dictionary));
    bytes.extend(block(1, &group));
    bytes
}

#[test]
fn authored_append_uses_actual_catalog_gap_for_small_declaration_properties() {
    let before = before();
    assert!(
        add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false)
            .unwrap()
            .can_add_keyed()
    );
    let advisory =
        authored_add_table_source_capabilities(&before, SCOPE, &mut budget(), &|| false).unwrap();
    assert!(advisory.can_add_keyed());
    assert!(!advisory.requires_native());

    let description = "D".repeat(255);
    let zero = Value::Int32(0);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
            .with_description(&description)
            .unwrap(),
    ];
    let declaration =
        FirstTableDefinition::capture_authored("Described", &columns, &mut budget(), &|| false)
            .unwrap();
    assert!(
        !declaration.is_native(),
        "the declaration fits the original canonical construction"
    );
    assert!(description.encode_utf16().count() * 2 > catalog_gap(&before));
    let image =
        NativeImage::capture_retained(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| false)
            .unwrap();
    let plan = image
        .plan_authored_add_table(declaration, &mut budget(), &|| false)
        .unwrap();
    assert!(plan.is_native());
    assert_eq!(plan.source_profile(), AddTableSourceProfile::Retained);
    assert_eq!(
        &plan.encode_intent(&mut budget(), &|| false).unwrap()[..8],
        b"TCAT0007"
    );
    assert_eq!(plan.before_bytes(), before);
    // The new two-field definition, DATA, primary root and property carrier
    // append; separate bitmap pages may also be required by actual map windows.
    assert!(plan.after_bytes().len() >= before.len() + 4 * 4096);
    for (number, page) in before.chunks_exact(4096).enumerate() {
        if [1, 2, 3, 4].contains(&number) {
            continue;
        }
        assert_eq!(
            &plan.after_bytes()[number * 4096..(number + 1) * 4096],
            page,
            "protected original page {number}"
        );
    }
    let observed = inspect_table_properties(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Described",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(observed.raw_bytes(), properties(&description));
    let group = observed.group(PropertyGroupKind::Column, "Score").unwrap();
    assert_eq!(
        group.get("Description").unwrap().text().unwrap(),
        description
    );
    assert!(!group.get("Required").unwrap().boolean().unwrap());
    assert_eq!(group.get("DefaultValue").unwrap().text().unwrap(), "0");
    let table = read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Described",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.columns().len(), 2);
    assert!(table.rows().is_empty());
}
