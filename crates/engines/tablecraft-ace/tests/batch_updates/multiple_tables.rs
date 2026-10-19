use super::*;

fn database() -> Vec<u8> {
    let mut bytes = base::database();
    bytes.resize(24 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    support::put32(&mut bytes, 2 * 4096 + 16, 4);
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        support::record(20, 0, 1, 0, b"\xff\xfeOther"),
    ];
    bytes[4 * 4096..5 * 4096].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    bytes[20 * 4096..21 * 4096].copy_from_slice(&base::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4)],
        3,
        21 << 8,
    ));
    bytes[21 * 4096..22 * 4096].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0, 0x40])],
    ));
    let rows = (1_i32..=3)
        .map(|id| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend((id * 100).to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[22 * 4096..23 * 4096].copy_from_slice(&support::data_page(20, &rows));
    bytes
}

#[test]
fn multi_table_batches_count_scoped_rows_and_reject_late_stale_values() {
    let bytes = database();
    let a = [FixedScalarUpdate::new(
        0,
        Value::Int32(10),
        Value::Int32(11),
    )];
    let b = [FixedScalarUpdate::new(
        0,
        Value::Int32(100),
        Value::Int32(101),
    )];
    let candidate = run(
        &bytes,
        &[
            FixedColumnUpdates::new("Items", "Score", &a),
            FixedColumnUpdates::new("Other", "Score", &b),
        ],
    )
    .unwrap();
    assert_eq!(candidate.changed_rows(), 2);
    checks(candidate.candidate_bytes(), "Items");
    checks(candidate.candidate_bytes(), "Other");
    assert_eq!(
        rows(candidate.candidate_bytes(), "Items")[0][1],
        Value::Int32(11)
    );
    assert_eq!(
        rows(candidate.candidate_bytes(), "Other")[0][1],
        Value::Int32(101)
    );
    let bad = [FixedScalarUpdate::new(
        0,
        Value::Int32(99),
        Value::Int32(101),
    )];
    assert_eq!(
        run(
            &bytes,
            &[
                FixedColumnUpdates::new("Items", "Score", &a),
                FixedColumnUpdates::new("Other", "Score", &bad)
            ]
        )
        .err(),
        Some(EditError::Conflict)
    );
    let reversed = run(
        &bytes,
        &[
            FixedColumnUpdates::new("Other", "Score", &b),
            FixedColumnUpdates::new("Items", "Score", &a),
        ],
    )
    .unwrap();
    assert_eq!(candidate.candidate_bytes(), reversed.candidate_bytes());
}
