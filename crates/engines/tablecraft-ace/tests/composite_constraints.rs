#[path = "mutation/fixture.rs"]
mod base;
mod support;

use std::io::Cursor;
use support::{data_page, put16, put32};
use tablecraft_ace::{
    ConstraintViolation, InspectError, RelationshipCheckStatus, check_table_constraints,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn row(ordinal: i32, crossed: bool) -> Vec<u8> {
    let mut bytes = 8_u16.to_le_bytes().to_vec();
    for value in [0_i32, 2, ordinal] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let names = [
        "SelfPair",
        "Items",
        "Items",
        if ordinal == 0 { "Id" } else { "Score" },
        if (ordinal == 0) ^ crossed {
            "Id"
        } else {
            "Score"
        },
    ];
    let mut offsets = vec![bytes.len() as u16];
    for name in names {
        bytes.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(bytes.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.push(0xff);
    bytes
}
fn database(crossed: bool) -> Vec<u8> {
    let mut bytes = base::with_rule("Caption", "composite");
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    put32(&mut bytes, 7 * 4096 + 16, 2);
    bytes[8 * 4096..9 * 4096].copy_from_slice(&data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 2])]));
    bytes[9 * 4096..10 * 4096]
        .copy_from_slice(&data_page(7, &[(0, row(0, crossed)), (0, row(1, crossed))]));
    let def = &mut bytes[10 * 4096..11 * 4096];
    def.copy_within(63..2000, 75);
    def[63..75].fill(0);
    put32(def, 47, 1);
    put32(def, 51, 1);
    let physical = 143;
    def[physical..physical + 80].fill(0);
    for i in 0..10 {
        put16(
            def,
            physical + 4 + 3 * i,
            if i < 2 { i as u16 } else { u16::MAX },
        );
        if i < 2 {
            def[physical + 6 + 3 * i] = 1;
        }
    }
    put32(def, physical + 38, 13);
    def[physical + 46] = 0x81;
    let name = "Pair"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    put16(def, 223, name.len() as u16);
    def[225..225 + name.len()].copy_from_slice(&name);
    bytes
}
fn audit(bytes: Vec<u8>) -> Result<tablecraft_ace::ConstraintReport, InspectError> {
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
}
#[test]
fn native_metadata_and_rows_drive_composite_audit_without_claiming_physical_index_validation() {
    let report = audit(database(false)).unwrap();
    assert_eq!(report.relationships().len(), 1);
    assert_eq!(
        report.relationships()[0].status(),
        RelationshipCheckStatus::Checked
    );
    assert_eq!(
        report.relationships()[0]
            .relationship()
            .declaration()
            .columns()
            .len(),
        2
    );
    assert_eq!(
        audit(database(true)).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
}
#[test]
fn absent_or_mismatched_unique_index_never_marks_composite_checked() {
    let mut bytes = database(false);
    bytes[10 * 4096 + 143 + 46] = 0x80;
    assert_eq!(
        audit(bytes).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Unsupported
    );
    let mut bytes = database(false);
    put16(&mut bytes, 10 * 4096 + 143 + 4, 1);
    put16(&mut bytes, 10 * 4096 + 143 + 7, 0);
    assert_eq!(
        audit(bytes).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Unsupported
    );
}
#[test]
fn duplicated_complete_parent_tuples_are_rejected_before_membership() {
    let mut bytes = database(false);
    let start0 = usize::from(u16::from_le_bytes(
        bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap(),
    ));
    let start1 = usize::from(u16::from_le_bytes(
        bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap(),
    ));
    let values = bytes[12 * 4096 + start0 + 2..12 * 4096 + start0 + 10].to_vec();
    bytes[12 * 4096 + start1 + 2..12 * 4096 + start1 + 10].copy_from_slice(&values);
    assert_eq!(
        audit(bytes).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}
