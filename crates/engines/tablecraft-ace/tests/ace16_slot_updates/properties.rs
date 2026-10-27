//! Original unindexed one-row field/property framing, independent of codecs.
use super::{budget, capture, fixture, support};
use std::io::Cursor;
use tablecraft_ace::{
    ConstraintViolation, EditError, InspectError, SlotScalarUpdate, check_table_constraints,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;

fn name(value: &str) -> Vec<u8> {
    let bytes = value
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    [(bytes.len() as u16).to_le_bytes().as_slice(), &bytes].concat()
}

fn block(kind: u16, bytes: &[u8]) -> Vec<u8> {
    [
        ((bytes.len() + 6) as u32).to_le_bytes().as_slice(),
        &kind.to_le_bytes(),
        bytes,
    ]
    .concat()
}

fn database(code: u8, key: &str, native_type: u8, property: &[u8], raw: &[u8]) -> Vec<u8> {
    let encoded = [
        ((property.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, native_type, 0, 0],
        &(property.len() as u16).to_le_bytes(),
        property,
    ]
    .concat();
    let label = name("Score");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &encoded,
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name(key)),
        &block(1, &group),
    ]
    .concat();

    let mut item = support::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    support::put16(&mut item, 0, 6);
    let property_start = item.len() as u16;
    item.extend_from_slice(&(0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend_from_slice(&[0; 8]);
    item.extend(properties);
    item.extend_from_slice(&(item.len() as u16).to_le_bytes());
    item.extend_from_slice(&property_start.to_le_bytes());
    item.extend_from_slice(&16_u16.to_le_bytes());
    item.extend_from_slice(&2_u16.to_le_bytes());
    item.push(0x3f);

    let mut bytes = fixture::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let catalog = support::data_page(
        2,
        &[
            (
                0,
                support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
            ),
            (
                0,
                support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog);

    let mut definition = fixture::definition(&[("Id", 4, 0, 4), ("Score", 10, 0, 510)], 1, 11 << 8);
    definition[99..103].copy_from_slice(&[9, 4, 0, 1]);
    definition[103] = 2;
    definition[104] = 1;
    bytes[10 * 4096..11 * 4096].copy_from_slice(&definition);
    let mut record = vec![2, 0, 1, 0, 0, 0];
    record.extend_from_slice(raw);
    record.extend_from_slice(&(record.len() as u16).to_le_bytes());
    record.extend_from_slice(&6_u16.to_le_bytes());
    record.extend_from_slice(&1_u16.to_le_bytes());
    record.push(3);
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &[(0, record)]));
    bytes
}

#[test]
fn actual_equal_width_values_still_obey_required_empty_and_logical_len_rules() {
    for code in [5, 6] {
        for (key, property, replacement, violation) in [
            (
                "Required",
                vec![0xff],
                Value::Null,
                FieldConstraintViolation::Required,
            ),
            (
                "AllowZeroLength",
                vec![0],
                Value::Text(String::new()),
                FieldConstraintViolation::EmptyText,
            ),
        ] {
            let bytes = database(code, key, 1, &property, &[0x41, 0]);
            check_table_constraints(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let result = capture(&bytes).preview_ace16_slot_updates(
                "Items",
                "Score",
                &[SlotScalarUpdate::new(
                    0,
                    Value::Text("A".into()),
                    replacement,
                )],
                &mut budget(),
                &|| false,
            );
            assert_eq!(
                result.err(),
                Some(EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::Field(violation)
                )))
            );
        }

        // Three wide units and four compressed ASCII units both occupy six
        // bytes. The after-image must still reject the changed logical Len.
        let rule = "Len([Score]) <= 3"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let bytes = database(code, "ValidationRule", 10, &rule, &[0, 1, 1, 1, 2, 1]);
        check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let result = capture(&bytes).preview_ace16_slot_updates(
            "Items",
            "Score",
            &[SlotScalarUpdate::new(
                0,
                Value::Text("ĀāĂ".into()),
                Value::Text("ABCD".into()),
            )],
            &mut budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(EditError::Inspect(InspectError::Constraint(
                ConstraintViolation::FieldRule
            )))
        );
    }
}
