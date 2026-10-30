//! Original literal native fixture owner, shared by actual tests.
use super::{data_page, image, put16, put32, record};

// Original independent golden format bytes. No external fixture/tool is needed.
pub fn database() -> Vec<u8> {
    let mut bytes = image(false, false);
    bytes.resize(14 * 4096, 0);
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    let catalog = [
        ("Id", 4, 0, 4),
        ("ParentId", 4, 4, 4),
        ("Name", 10, 0, 510),
        ("Type", 3, 8, 2),
        ("Flags", 4, 10, 4),
        ("LvProp", 11, 0, 0),
        ("Lv", 11, 0, 0),
        ("LvExtra", 11, 0, 0),
        ("LvModule", 11, 0, 0),
    ];
    put_page(&mut bytes, 2, &definition(&catalog, 3, 3 << 8));
    let objects = [
        record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        record(10, 0, 1, 0, b"\xff\xfeItems"),
    ];
    put_page(
        &mut bytes,
        4,
        &data_page(
            2,
            &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
        ),
    );
    let relationships = [
        ("szRelationship", 10, 0, 510),
        ("grbit", 4, 0, 4),
        ("ccolumn", 4, 4, 4),
        ("icolumn", 4, 8, 4),
        ("szReferencedObject", 10, 0, 510),
        ("szObject", 10, 0, 510),
        ("szReferencedColumn", 10, 0, 510),
        ("szColumn", 10, 0, 510),
    ];
    put_page(&mut bytes, 7, &definition(&relationships, 0, 8 << 8));
    put_page(
        &mut bytes,
        8,
        &data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0])]),
    );
    put_page(
        &mut bytes,
        10,
        &definition(&[("Id", 4, 0, 4), ("Score", 4, 4, 4)], 3, 11 << 8),
    );
    put_page(
        &mut bytes,
        11,
        &data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x10])]),
    );
    let rows = [(1_i32, 10_i32), (2, 20), (3, 30)]
        .into_iter()
        .map(|(id, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend_from_slice(&id.to_le_bytes());
            row.extend_from_slice(&score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    put_page(&mut bytes, 12, &data_page(10, &rows));
    bytes[13 * 4096..].fill(0xa7);
    bytes
}

fn put_page(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * 4096..(number + 1) * 4096].copy_from_slice(page);
}

pub fn with_rule(key: &str, text: &str) -> Vec<u8> {
    let name = |value: &str| {
        let data = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        [(data.len() as u16).to_le_bytes().as_slice(), &data].concat()
    };
    let block = |kind: u16, body: &[u8]| {
        [
            ((body.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    };
    let data = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let property = [
        ((data.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, 10, 0, 0],
        &(data.len() as u16).to_le_bytes(),
        &data,
    ]
    .concat();
    let label = name("Score");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &property,
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name(key)),
        &block(1, &group),
    ]
    .concat();
    let mut item = record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    put16(&mut item, 0, 6);
    let property_start = item.len() as u16;
    item.extend_from_slice(&(0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend_from_slice(&[0; 8]);
    item.extend(properties);
    item.extend_from_slice(&(item.len() as u16).to_le_bytes());
    item.extend_from_slice(&property_start.to_le_bytes());
    item.extend_from_slice(&16_u16.to_le_bytes());
    item.extend_from_slice(&2_u16.to_le_bytes());
    item.push(0x3f);
    let mut bytes = database();
    put_page(
        &mut bytes,
        4,
        &data_page(
            2,
            &[
                (0, record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
                (0, record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships")),
                (0, item),
            ],
        ),
    );
    bytes
}

pub fn definition(fields: &[(&str, u8, u16, u16)], rows: u32, usage: u32) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[2, 1]);
    put32(&mut page, 8, 4096);
    put32(&mut page, 16, rows);
    put16(&mut page, 41, fields.len() as u16);
    put16(&mut page, 45, fields.len() as u16);
    put32(&mut page, 55, usage);
    let mut names = 63 + 25 * fields.len();
    let mut variables = 0;
    for (number, (name, kind, offset, length)) in fields.iter().enumerate() {
        let start = 63 + 25 * number;
        page[start] = *kind;
        put16(&mut page, start + 5, number as u16);
        if matches!(*kind, 10 | 11) {
            put16(&mut page, start + 7, variables);
            variables += 1;
        } else {
            page[start + 15] = 3;
        }
        put16(&mut page, start + 21, *offset);
        put16(&mut page, start + 23, *length);
        let text = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut page, names, text.len() as u16);
        names += 2;
        page[names..names + text.len()].copy_from_slice(&text);
        names += text.len();
    }
    put16(&mut page, 43, variables);
    page
}
