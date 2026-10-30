//! Independent donor-shaped source and one-bit expected image; never donated data.
use super::{base, complex_fixture, modern_attachment, raw};

#[path = "model/properties.rs"]
mod properties;
#[path = "model/records.rs"]
mod records;
#[path = "model/schema.rs"]
mod schema;

pub const PAGE: usize = 4096;
pub const BOOLEAN_PATCH_OFFSET: usize = 13 * PAGE - 1;

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn complete_tail(bytes: &mut [u8], definition: usize) {
    let page = &mut bytes[definition * PAGE..(definition + 1) * PAGE];
    let count = u16::from_le_bytes(page[45..47].try_into().unwrap()) as usize;
    let mut tail = 63 + count * 25;
    for _ in 0..count {
        let length = u16::from_le_bytes(page[tail..tail + 2].try_into().unwrap()) as usize;
        tail += 2 + length;
    }
    raw::put16(page, tail, 0xffff);
}

fn property_record() -> Vec<u8> {
    let mut row = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    row.truncate(row.len() - 7);
    raw::put16(&mut row, 0, 6);
    let property_start = row.len() as u16;
    row.extend(records::inline(&properties::bag()));
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(property_start.to_le_bytes());
    row.extend(16_u16.to_le_bytes());
    row.extend(2_u16.to_le_bytes());
    row.push(0x3f);
    row
}

fn mapping_record() -> Vec<u8> {
    let mut row = complex_fixture::mapping(7, 10, 20, 17);
    row.truncate(18);
    row.extend(records::text("Attachment_Field"));
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(18_u16.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(0x1f);
    row
}

pub fn source() -> Case {
    let mut before = modern_attachment::database(6);
    before.resize(32 * PAGE, 0);
    before[31 * PAGE..32 * PAGE].fill(0xc7);

    let catalog_rows = [
        ("MSysObjects", 2),
        ("MSysRelationships", 7),
        ("Items", 10),
        ("MSysComplexColumns", 14),
        ("ValueType", 17),
        ("Children", 20),
    ]
    .into_iter()
    .map(|(name, number)| {
        let row = if number == 10 {
            property_record()
        } else {
            raw::record(
                number,
                0,
                1,
                0,
                &[b"\xff\xfe".as_slice(), name.as_bytes()].concat(),
            )
        };
        (0, row)
    })
    .collect::<Vec<_>>();
    install(&mut before, 4, &records::page(2, &catalog_rows));
    install(&mut before, 10, &schema::definition());
    install(
        &mut before,
        12,
        &records::page(
            10,
            &(0..3)
                .map(|ordinal| (0, records::record(ordinal)))
                .collect::<Vec<_>>(),
        ),
    );
    install(
        &mut before,
        16,
        &records::page(14, &[(0, mapping_record())]),
    );

    for (definition, map, members) in [
        (2, 3, vec![4]),
        (7, 8, vec![]),
        (14, 15, vec![16]),
        (17, 18, vec![]),
        (20, 21, vec![22]),
    ] {
        complete_tail(&mut before, definition);
        raw::put32(&mut before, definition * PAGE + 59, ((map as u32) << 8) | 1);
        install(
            &mut before,
            map,
            &records::page(
                0,
                &[(0, records::map(&members)), (0, records::map(&members))],
            ),
        );
    }
    let map_members = [
        vec![12],
        vec![12],
        vec![24],
        vec![24],
        vec![25],
        vec![25],
        vec![26],
        vec![26],
    ];
    let map_rows = map_members
        .iter()
        .map(|members| (0, records::map(members)))
        .collect::<Vec<_>>();
    install(&mut before, 11, &records::page(0, &map_rows));
    install(&mut before, 24, &records::lval(&records::text("memo")));
    install(&mut before, 25, &records::lval(&[0, 255, 7]));
    install(
        &mut before,
        26,
        &records::lval(&records::text("label#https://invalid.test/#")),
    );

    let mut indexes = Vec::new();
    for physical in 0..3 {
        install(&mut before, 27 + physical, &schema::index_page(physical));
        indexes.push((0, records::map(&[27 + physical])));
    }
    install(&mut before, 30, &records::page(0, &indexes));
    install(
        &mut before,
        1,
        &records::page(1, &[(0, records::map(&(32..512).collect::<Vec<_>>()))]),
    );

    let mut expected = before.clone();
    expected[BOOLEAN_PATCH_OFFSET] ^= 0x04;
    assert_eq!(before[BOOLEAN_PATCH_OFFSET] & 0x04, 0);
    assert_eq!(expected[BOOLEAN_PATCH_OFFSET] & 0x04, 0x04);
    Case { before, expected }
}
