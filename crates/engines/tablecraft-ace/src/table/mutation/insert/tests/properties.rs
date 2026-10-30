//! An explicit typed empty-Text policy, never an inferred database default.
use tablecraft_testkit::access as raw;
use tablecraft_testkit::access::insert_seed::PAGE;

fn name(text: &str) -> Vec<u8> {
    let bytes = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    [(bytes.len() as u16).to_le_bytes().as_slice(), &bytes].concat()
}

fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    [
        ((body.len() + 6) as u32).to_le_bytes().as_slice(),
        &kind.to_le_bytes(),
        body,
    ]
    .concat()
}

pub(super) fn allow_empty_text(bytes: &mut [u8]) {
    let mut properties = b"MR2\0".to_vec();
    properties.extend(block(0x80, &name("AllowZeroLength")));
    let label = name("Score");
    let group = [
        ((label.len() + 4) as u32).to_le_bytes().as_slice(),
        &label,
        &[9, 0, 0, 1, 0, 0, 1, 0, 255],
    ]
    .concat();
    properties.extend(block(1, &group));

    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    raw::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x80000000_u32 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend([16, 0, 2, 0, 0x3f]);

    let catalog = raw::data_page(
        2,
        &[
            (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
            (
                0,
                raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&catalog);
}
