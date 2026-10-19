use super::*;

fn database() -> Vec<u8> {
    let name = |text: &str| {
        let text = text
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        [(text.len() as u16).to_le_bytes().as_slice(), &text].concat()
    };
    let block = |kind: u16, body: &[u8]| {
        [
            ((body.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    };
    let text = "[Id] < [Score]"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let property = [
        ((text.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, 10, 0, 0],
        &(text.len() as u16).to_le_bytes(),
        &text,
    ]
    .concat();
    let group = [
        6_u32.to_le_bytes().as_slice(),
        &0_u16.to_le_bytes(),
        &property,
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name("ValidationRule")),
        &block(0, &group),
    ]
    .concat();
    let mut item = support::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    support::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
    let mut bytes = base::database();
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        item,
    ];
    bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    bytes
}

#[test]
fn cross_field_table_rules_check_the_complete_row_after_all_assignments() {
    use tablecraft_ace::{ConstraintViolation, InspectError};
    let bytes = database();
    checks(&bytes, "Items");
    let first = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(20))];
    let second = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(30),
    )];
    assert_eq!(
        run(&bytes, &[FixedColumnUpdates::new("Items", "Id", &first)]).err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::TableRule
        )))
    );
    let candidate = run(
        &bytes,
        &[
            FixedColumnUpdates::new("Items", "Id", &first),
            FixedColumnUpdates::new("Items", "Score", &second),
        ],
    )
    .unwrap();
    assert_eq!(candidate.changed_rows(), 1);
    assert_eq!(
        rows(candidate.candidate_bytes(), "Items")[0],
        vec![Value::Int32(20), Value::Int32(30)]
    );
    checks(candidate.candidate_bytes(), "Items");
    let invalid = [FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(40))];
    assert_eq!(
        run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Id", &invalid),
                FixedColumnUpdates::new("Items", "Score", &second)
            ]
        )
        .err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::TableRule
        )))
    );
}
