use super::{
    base,
    support::{data_page, put16, put32, record},
};

fn page(bytes: &mut [u8], number: usize, data: &[u8]) {
    bytes[number * 4096..(number + 1) * 4096].copy_from_slice(data);
}
pub fn mapping(type_id: i32, owner: i32, flat: i32, template: i32) -> Vec<u8> {
    let mut row = 5_u16.to_le_bytes().to_vec();
    for n in [type_id, owner, flat, template] {
        row.extend_from_slice(&n.to_le_bytes());
    }
    let start = row.len() as u16;
    row.extend("Score".encode_utf16().flat_map(u16::to_le_bytes));
    row.extend_from_slice(&(row.len() as u16).to_le_bytes());
    row.extend_from_slice(&start.to_le_bytes());
    row.extend_from_slice(&1_u16.to_le_bytes());
    row.push(0x1f);
    row
}
pub fn database() -> Vec<u8> {
    let mut bytes = base::database();
    bytes.resize(24 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    put32(&mut bytes, 2 * 4096 + 16, 6);
    let objects = [
        ("MSysObjects", 2),
        ("MSysRelationships", 7),
        ("Items", 10),
        ("MSysComplexColumns", 14),
        ("ValueType", 17),
        ("Children", 20),
    ];
    let rows = objects
        .iter()
        .map(|(name, id)| {
            (
                0,
                record(
                    *id,
                    0,
                    1,
                    0,
                    &[b"\xff\xfe".as_slice(), name.as_bytes()].concat(),
                ),
            )
        })
        .collect::<Vec<_>>();
    page(&mut bytes, 4, &data_page(2, &rows));
    bytes[10 * 4096 + 63 + 25] = 0x12;
    put32(&mut bytes, 10 * 4096 + 63 + 25 + 11, 7);
    page(
        &mut bytes,
        14,
        &base::definition(
            &[
                ("ComplexID", 4, 0, 4),
                ("ConceptualTableID", 4, 4, 4),
                ("ColumnName", 10, 0, 510),
                ("FlatTableID", 4, 8, 4),
                ("ComplexTypeObjectID", 4, 12, 4),
            ],
            1,
            15 << 8,
        ),
    );
    page(
        &mut bytes,
        15,
        &data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0, 1])]),
    );
    page(
        &mut bytes,
        16,
        &data_page(14, &[(0, mapping(7, 10, 20, 17))]),
    );
    page(
        &mut bytes,
        17,
        &base::definition(&[("Value", 4, 0, 4)], 0, 18 << 8),
    );
    page(&mut bytes, 18, &data_page(1, &[(0, vec![0, 0, 0, 0, 0])]));
    page(
        &mut bytes,
        20,
        &base::definition(
            &[("Owner", 4, 0, 4), ("Value", 4, 4, 4), ("ChildID", 4, 8, 4)],
            0,
            21 << 8,
        ),
    );
    page(
        &mut bytes,
        21,
        &data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0, 0x40])]),
    );
    page(&mut bytes, 22, &data_page(20, &[]));
    bytes[20 * 4096 + 63 + 16] = 0x08;
    bytes[20 * 4096 + 63 + 50 + 15] = 0x07;
    put16(&mut bytes, 10 * 4096 + 63 + 25 + 23, 4);
    bytes
}
pub fn with_mappings(rows: Vec<Vec<u8>>) -> Vec<u8> {
    let mut bytes = database();
    put32(&mut bytes, 14 * 4096 + 16, rows.len() as u32);
    let rows = rows.into_iter().map(|r| (0, r)).collect::<Vec<_>>();
    page(&mut bytes, 16, &data_page(14, &rows));
    bytes
}
