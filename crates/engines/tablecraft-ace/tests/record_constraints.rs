use std::io::Cursor;
use tablecraft_ace::{
    ConstraintViolation, InspectError, RelationshipCheckStatus, RuleCheckStatus,
    check_table_constraints, inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

fn copy() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}
fn audit(bytes: Vec<u8>, name: &str) -> Result<tablecraft_ace::ConstraintReport, InspectError> {
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        name,
        &mut budget(),
        &|| false,
    )
}
// Independently parse live simple data roots to mutate disposable fixture copies.
// This is test-only geometry, deliberately independent of production constants.
fn roots(bytes: &[u8], owner: u32) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    for (number, page) in bytes.as_chunks::<4096>().0.iter().enumerate() {
        if page[..2] != [1, 1] || u32::from_le_bytes(page[4..8].try_into().unwrap()) != owner {
            continue;
        }
        let count = u16::from_le_bytes(page[12..14].try_into().unwrap()) as usize;
        for slot in 0..count {
            let raw = u16::from_le_bytes(page[14 + slot * 2..16 + slot * 2].try_into().unwrap());
            if raw & 0xe000 != 0 {
                continue;
            }
            let start = usize::from(raw & 0x1fff);
            let end = if slot == 0 {
                4096
            } else {
                usize::from(
                    u16::from_le_bytes(page[12 + slot * 2..14 + slot * 2].try_into().unwrap())
                        & 0x1fff,
                )
            };
            rows.push((number * 4096 + start, number * 4096 + end));
        }
    }
    rows
}
fn definition(bytes: &[u8], name: &str) -> u32 {
    inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        name,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .definition_page()
}

#[test]
fn real_field_and_foreign_key_declarations_have_explicit_record_coverage() {
    let bytes = copy();
    let report = audit(bytes.clone(), "TAG_NME_TB").unwrap();
    assert_eq!(report.rows(), 933);
    assert_eq!(
        report.relationships()[0].status(),
        RelationshipCheckStatus::Checked
    );
    assert_eq!(
        report
            .fields()
            .iter()
            .find(|field| field.name() == "TAG_GRP_ID")
            .unwrap()
            .required(),
        RuleCheckStatus::Checked
    );
    assert_eq!(copy(), bytes);
}

#[test]
fn required_and_empty_text_violations_are_distinct_typed_failures() {
    let original = copy();
    let (start, end) = roots(&original, 726)[0];
    assert_eq!(
        u16::from_le_bytes(original[start..start + 2].try_into().unwrap()),
        3
    );
    let mut bytes = original.clone();
    bytes[end - 1] &= !2;
    assert_eq!(
        audit(bytes, "ROULETTE_TB").err(),
        Some(InspectError::Constraint(ConstraintViolation::Field(
            FieldConstraintViolation::Required
        )))
    );
    let mut bytes = original.clone();
    let count_position = end - 3;
    assert_eq!(
        u16::from_le_bytes(
            bytes[count_position..count_position + 2]
                .try_into()
                .unwrap()
        ),
        1
    );
    let start_offset = [bytes[count_position - 4], bytes[count_position - 3]];
    bytes[count_position - 2..count_position].copy_from_slice(&start_offset);
    assert_eq!(
        audit(bytes, "ROULETTE_TB").err(),
        Some(InspectError::Constraint(ConstraintViolation::Field(
            FieldConstraintViolation::EmptyText
        )))
    );
    assert_eq!(copy(), original);
}

#[test]
fn orphan_children_and_duplicate_referenced_keys_are_detected_sequentially() {
    let original = copy();
    let child = definition(&original, "TAG_NME_TB");
    let (row, _) = roots(&original, child)[0];
    let mut bytes = original.clone();
    bytes[row + 6..row + 10].copy_from_slice(&i32::MAX.to_le_bytes());
    assert_eq!(
        audit(bytes, "TAG_NME_TB").err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    let parent = definition(&original, "TAG_GRP_TB");
    let rows = roots(&original, parent);
    let key = original[rows[0].0 + 2..rows[0].0 + 6].to_vec();
    let mut bytes = original.clone();
    bytes[rows[1].0 + 2..rows[1].0 + 6].copy_from_slice(&key);
    assert_eq!(
        audit(bytes, "TAG_GRP_TB").err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn record_audits_are_cancellable_and_share_operation_limits() {
    assert!(matches!(
        check_table_constraints(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        check_table_constraints(
            &mut Cursor::new(copy()),
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
