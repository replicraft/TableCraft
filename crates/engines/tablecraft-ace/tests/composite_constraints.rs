#[path = "mutation/fixture.rs"]
mod base;
#[path = "composite_constraints/guid.rs"]
mod guid;
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

fn mixed_database(crossed: bool) -> Vec<u8> {
    let mut bytes = database(crossed);
    bytes[10 * 4096 + 75 + 25] = 0x13;
    put16(&mut bytes, 10 * 4096 + 75 + 25 + 23, 8);
    let rows = [i64::MIN, 9_007_199_254_740_993, i64::MAX]
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend_from_slice(&(index as i32 + 1).to_le_bytes());
            row.extend_from_slice(&value.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    bytes
}

#[test]
fn mixed_integer_composite_keys_are_checked_with_matching_pair_types() {
    for code in [5, 6] {
        let mut bytes = mixed_database(false);
        bytes[20] = code;
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            RelationshipCheckStatus::Checked
        );
    }
    // All fields are supported, but crossed Int32/Int64 declarations are not
    // implicitly coerced even when values could happen to be equal.
    assert_eq!(
        audit(mixed_database(true)).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Unsupported
    );
}

#[test]
fn mixed_integer_parent_uniqueness_is_exact() {
    let mut bytes = mixed_database(false);
    let first = usize::from(u16::from_le_bytes(
        bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap(),
    ));
    let second = usize::from(u16::from_le_bytes(
        bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap(),
    ));
    let values = bytes[12 * 4096 + first + 2..12 * 4096 + first + 14].to_vec();
    bytes[12 * 4096 + second + 2..12 * 4096 + second + 14].copy_from_slice(&values);
    assert_eq!(
        audit(bytes).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

fn narrow_database(crossed: bool, values: &[(Option<u8>, Option<i16>)]) -> Vec<u8> {
    let mut bytes = database(crossed);
    let first = 10 * 4096 + 75;
    let second = first + 25;
    bytes[first] = 2;
    put16(&mut bytes, first + 23, 1);
    bytes[second] = 3;
    put16(&mut bytes, second + 21, 1);
    put16(&mut bytes, second + 23, 2);
    put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    let rows = values
        .iter()
        .map(|(a, b)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.push(a.unwrap_or(0));
            row.extend(b.unwrap_or(0).to_le_bytes());
            row.push(u8::from(a.is_some()) | u8::from(b.is_some()) << 1);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    bytes
}

#[test]
fn narrow_integer_tuples_keep_unsigned_values_partial_nulls_and_pair_type_gates() {
    let values = [
        (Some(128), Some(i16::MIN)),
        (Some(255), Some(i16::MAX)),
        (Some(0), Some(0)),
        (None, Some(i16::MAX)),
        (Some(255), None),
        (None, None),
        (None, None),
    ];
    for code in [5, 6] {
        let mut bytes = narrow_database(false, &values);
        bytes[20] = code;
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            RelationshipCheckStatus::Checked
        );
        let mut crossed = narrow_database(true, &values);
        crossed[20] = code;
        assert_eq!(
            audit(crossed).unwrap().relationships()[0].status(),
            RelationshipCheckStatus::Unsupported
        );
    }
    assert_eq!(
        audit(narrow_database(false, &[values[0], values[0]])).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn crossed_same_type_narrow_keys_fail_membership_without_signed_byte_coercion() {
    for small in [false, true] {
        for crossed in [false, true] {
            let mut bytes = narrow_database(crossed, &[(Some(0), Some(0)); 2]);
            let first = 10 * 4096 + 75;
            let second = first + 25;
            let (kind, width) = if small { (3, 2) } else { (2, 1) };
            bytes[first] = kind;
            bytes[second] = kind;
            put16(&mut bytes, first + 23, width);
            put16(&mut bytes, second + 23, width);
            put16(&mut bytes, second + 21, width);
            let values = if small {
                [(i16::MIN, i16::MAX), (-1, 1)]
            } else {
                [(128, 255), (0, 127)]
            };
            let rows = values
                .into_iter()
                .map(|(a, b)| {
                    let mut row = 2_u16.to_le_bytes().to_vec();
                    for value in [a, b] {
                        if small {
                            row.extend(value.to_le_bytes());
                        } else {
                            row.push(value as u8);
                        }
                    }
                    row.push(3);
                    (0, row)
                })
                .collect::<Vec<_>>();
            bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
            if crossed {
                assert_eq!(
                    audit(bytes).err(),
                    Some(InspectError::Constraint(ConstraintViolation::MissingParent))
                );
            } else {
                assert_eq!(
                    audit(bytes).unwrap().relationships()[0].status(),
                    RelationshipCheckStatus::Checked
                );
            }
        }
    }
}
