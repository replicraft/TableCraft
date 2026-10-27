use super::*;

pub const PAGE: usize = 4096;
pub const ROOT: usize = 12;
pub const MEMO: usize = 13;
pub const OLE: usize = 14;
pub const EOF: usize = 24;
pub const SLOT: usize = 14;
pub const OFFSET_MASK: u16 = 0x1fff;
pub const HIDDEN: u16 = 0x8000;
pub const FORWARDED: u16 = 0x4000;
pub const MAP_HEADER: usize = 5;
pub const DECLARATIONS: usize = 162;
pub const MEMO_FLAGS: usize = 103;
pub const MEMO_EXTENDED_FLAGS: usize = 104;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 50_000_000)
}

pub fn page(owner: u32, rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = support::data_page(owner, rows);
    let body: usize = rows.iter().map(|(_, row)| row.len()).sum();
    support::put16(&mut bytes, 2, (PAGE - SLOT - 2 * rows.len() - body) as u16);
    bytes
}

pub fn lval(rows: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = page(0, rows);
    bytes[4..8].copy_from_slice(b"LVAL");
    bytes
}

pub fn map(numbers: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; 69];
    for &number in numbers {
        bytes[MAP_HEADER + number / 8] |= 1 << (number % 8);
    }
    bytes
}

pub fn descriptor(tag: u32, payload_bytes: usize, reference: u32) -> Vec<u8> {
    let mut bytes = (tag | payload_bytes as u32).to_le_bytes().to_vec();
    bytes.extend(reference.to_le_bytes());
    bytes.extend([0; 4]);
    bytes
}

pub fn single(length: usize, page: usize, slot: usize) -> Vec<u8> {
    descriptor(0x4000_0000, length, ((page << 8) | slot) as u32)
}

pub fn inline(payload: &[u8]) -> Vec<u8> {
    let mut bytes = descriptor(0x8000_0000, payload.len(), 0);
    bytes.extend(payload);
    bytes
}

pub fn record(id: i32, memo: Option<&[u8]>, ole: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = vec![3, 0];
    bytes.extend(id.to_le_bytes());
    if let Some(value) = memo {
        bytes.extend(value);
    }
    let middle = bytes.len() as u16;
    if let Some(value) = ole {
        bytes.extend(value);
    }
    let end = bytes.len() as u16;
    bytes.extend(end.to_le_bytes());
    bytes.extend(middle.to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.push(1 | if memo.is_some() { 2 } else { 0 } | if ole.is_some() { 4 } else { 0 });
    bytes
}

pub fn install(bytes: &mut [u8], number: usize, contents: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(contents);
}

pub fn set_maps(bytes: &mut [u8], memo_owned: &[usize], memo_hints: &[usize], ole_owned: &[usize]) {
    let rows = vec![
        (0, map(&[ROOT])),
        (0, map(&[ROOT])),
        (0, map(memo_owned)),
        (0, map(memo_hints)),
        (0, map(ole_owned)),
        (0, map(ole_owned)),
    ];
    install(bytes, 11, &page(0, &rows));
}

pub fn source(code: u8, roots: usize) -> Vec<u8> {
    let mut bytes = fixture::database();
    bytes.resize(EOF * PAGE, 0);
    // This fresh literal fixture has no system-column orphan ownership. Give
    // both complete declarations, rather than asking a writer to infer that
    // zero-filled unknown suffixes mean an empty historical inventory.
    let catalog_fields = [
        "Id", "ParentId", "Name", "Type", "Flags", "LvProp", "Lv", "LvExtra", "LvModule",
    ];
    let relationship_fields = [
        "szRelationship",
        "grbit",
        "ccolumn",
        "icolumn",
        "szReferencedObject",
        "szObject",
        "szReferencedColumn",
        "szColumn",
    ];
    for (number, names) in [
        (2, catalog_fields.as_slice()),
        (7, relationship_fields.as_slice()),
    ] {
        let names_bytes: usize = names
            .iter()
            .map(|name| 2 + name.encode_utf16().count() * 2)
            .sum();
        let tail = 63 + names.len() * 25 + names_bytes;
        assert_eq!(tail, if number == 2 { 398 } else { 449 });
        support::put16(&mut bytes, number * PAGE + tail, u16::MAX);
    }
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    let mut definition = fixture::definition(
        &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Ole", 11, 0, 0)],
        roots as u32,
        11 << 8,
    );
    definition[88] = 12;
    definition[103] = 2;
    definition[128] = 2;
    support::put32(&mut definition, 59, (11 << 8) | 1);
    for (at, column, owned, hint) in [
        (DECLARATIONS, 1, (11 << 8) | 2, (11 << 8) | 3),
        (DECLARATIONS + 10, 2, (11 << 8) | 4, (11 << 8) | 5),
    ] {
        support::put16(&mut definition, at, column);
        support::put32(&mut definition, at + 2, owned);
        support::put32(&mut definition, at + 6, hint);
    }
    support::put16(&mut definition, DECLARATIONS + 20, u16::MAX);
    install(&mut bytes, 10, &definition);
    set_maps(&mut bytes, &[MEMO], &[MEMO], &[OLE]);
    let memo = single(2, MEMO, 0);
    let ole = single(1, OLE, 0);
    let rows = (0..roots)
        .map(|ordinal| (0, record(ordinal as i32 + 1, Some(&memo), Some(&ole))))
        .collect::<Vec<_>>();
    install(&mut bytes, ROOT, &page(10, &rows));
    install(
        &mut bytes,
        MEMO,
        &lval(&[(0, vec![65, 0]), (0, vec![0xf1, 0xe2, 0xd3])]),
    );
    install(&mut bytes, OLE, &lval(&[(0, vec![7])]));
    let free: Vec<_> = (EOF..512).collect();
    install(&mut bytes, 1, &page(1, &[(0, map(&free))]));
    bytes
}

pub fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn check(bytes: &[u8]) -> tablecraft_ace::TableLongValueCheck {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    check_table_long_values(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub fn read(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

pub fn run(
    bytes: &[u8],
    column: &str,
    updates: &[SlotScalarUpdate],
) -> Result<Ace16WritePreview, EditError> {
    capture(bytes).preview_ace16_long_value_updates(
        "Items",
        column,
        updates,
        &mut budget(),
        &|| false,
    )
}

pub fn word(bytes: &[u8], page: usize, slot: usize) -> u16 {
    let at = page * PAGE + SLOT + 2 * slot;
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

pub fn body(bytes: &[u8], page: usize, slot: usize) -> &[u8] {
    let start = page * PAGE + usize::from(word(bytes, page, slot) & OFFSET_MASK);
    let end = if slot == 0 {
        (page + 1) * PAGE
    } else {
        page * PAGE + usize::from(word(bytes, page, slot - 1) & OFFSET_MASK)
    };
    &bytes[start..end]
}

pub fn map_byte(bytes: &[u8], row: usize, number: usize) -> u8 {
    let start = usize::from(word(bytes, 11, row) & OFFSET_MASK);
    bytes[11 * PAGE + start + MAP_HEADER + number / 8]
}

pub fn old_chunks_unchanged(before: &[u8], after: &[u8]) {
    for (page, count) in [(MEMO, 2), (OLE, 1)] {
        for slot in 0..count {
            assert_eq!(word(before, page, slot), word(after, page, slot));
            assert_eq!(body(before, page, slot), body(after, page, slot));
        }
    }
}

pub fn refused(result: Result<Ace16WritePreview, EditError>) {
    assert!(matches!(
        result,
        Err(EditError::Unsupported(_)) | Err(EditError::Inspect(InspectError::Unsupported))
    ));
}

pub fn memo_update(value: Value) -> SlotScalarUpdate {
    SlotScalarUpdate::new(0, Value::Text("A".into()), value)
}

pub fn selected_property(bytes: &mut [u8], key: &str) {
    property(bytes, Some("Memo"), key, "passive");
}

pub fn table_rule(bytes: &mut [u8], rule: &str) {
    property(bytes, None, "ValidationRule", rule);
}

fn property(bytes: &mut [u8], column: Option<&str>, key: &str, text: &str) {
    fn name(value: &str) -> Vec<u8> {
        let text: Vec<u8> = value.encode_utf16().flat_map(u16::to_le_bytes).collect();
        [(text.len() as u16).to_le_bytes().as_slice(), &text].concat()
    }
    fn block(kind: u16, body: &[u8]) -> Vec<u8> {
        [
            ((body.len() + 6) as u32).to_le_bytes().as_slice(),
            &kind.to_le_bytes(),
            body,
        ]
        .concat()
    }
    let data: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let property = [
        ((data.len() + 8) as u16).to_le_bytes().as_slice(),
        &[0, 10, 0, 0],
        &(data.len() as u16).to_le_bytes(),
        &data,
    ]
    .concat();
    let owner = name(column.unwrap_or(""));
    let group = [
        ((owner.len() + 4) as u32).to_le_bytes().as_slice(),
        &owner,
        &property,
    ]
    .concat();
    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name(key)),
        &block(u16::from(column.is_some()), &group),
    ]
    .concat();
    let mut item = support::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    support::put16(&mut item, 0, 6);
    let start = item.len() as u16;
    item.extend((0x8000_0000 | properties.len() as u32).to_le_bytes());
    item.extend([0; 8]);
    item.extend(properties);
    item.extend((item.len() as u16).to_le_bytes());
    item.extend(start.to_le_bytes());
    item.extend(16_u16.to_le_bytes());
    item.extend(2_u16.to_le_bytes());
    item.push(0x3f);
    install(
        bytes,
        4,
        &page(
            2,
            &[
                (
                    0,
                    support::record(2, 0, 1, 0x8000_0002, b"\xff\xfeMSysObjects"),
                ),
                (
                    0,
                    support::record(7, 0, 1, 0x8000_0002, b"\xff\xfeMSysRelationships"),
                ),
                (0, item),
            ],
        ),
    );
}

fn set_member(bytes: &mut [u8], row: usize, number: usize, set: bool) {
    let start = usize::from(word(bytes, 11, row) & OFFSET_MASK);
    let at = 11 * PAGE + start + MAP_HEADER + number / 8;
    let mask = 1 << (number % 8);
    if set {
        bytes[at] |= mask;
    } else {
        bytes[at] &= !mask;
    }
}

pub fn combined_expected(before: &[u8]) -> Vec<u8> {
    let mut expected = before.to_vec();
    expected.resize((EOF + 4) * PAGE, 0);
    let sizes = [4072, 4072, 2];
    for (position, size) in sizes.into_iter().enumerate() {
        let mut chunk = if position < 2 {
            (((EOF + position + 1) as u32) << 8).to_le_bytes().to_vec()
        } else {
            0_u32.to_le_bytes().to_vec()
        };
        chunk.extend([0, 1].repeat(size / 2));
        install(&mut expected, EOF + position, &lval(&[(0, chunk)]));
        set_member(&mut expected, 2, EOF + position, true);
    }
    set_member(&mut expected, 3, MEMO, false);
    set_member(&mut expected, 3, EOF + 2, true);
    set_member(&mut expected, 0, EOF + 3, true);
    set_member(&mut expected, 1, ROOT, false);
    set_member(&mut expected, 1, EOF + 3, true);
    let descriptor = descriptor(0, 8146, (EOF as u32) << 8);
    let ole = single(1, OLE, 0);
    let target = record(1, Some(&descriptor), Some(&ole));
    assert_eq!(target.len(), 39);
    install(&mut expected, EOF + 3, &page(10, &[(HIDDEN, target)]));
    let start = ROOT * PAGE + usize::from(word(before, ROOT, 0) & OFFSET_MASK);
    let original_len = body(before, ROOT, 0).len();
    expected[start..start + original_len].fill(0);
    expected[start..start + 4].copy_from_slice(&(((EOF + 3) as u32) << 8).to_le_bytes());
    support::put16(
        &mut expected,
        ROOT * PAGE + SLOT,
        word(before, ROOT, 0) | FORWARDED,
    );
    let global_start = usize::from(word(before, 1, 0) & OFFSET_MASK);
    expected[PAGE + global_start + MAP_HEADER + EOF / 8] &= !0x0f;
    expected
}

pub fn other_table_alias(bytes: &mut [u8], declared: bool, external: bool) {
    // New independent catalog object uses existing allocated pages15..17.
    support::put32(bytes, 2 * PAGE + 16, 4);
    install(
        bytes,
        4,
        &page(
            2,
            &[
                (
                    0,
                    support::record(2, 0, 1, 0x8000_0002, b"\xff\xfeMSysObjects"),
                ),
                (
                    0,
                    support::record(7, 0, 1, 0x8000_0002, b"\xff\xfeMSysRelationships"),
                ),
                (0, support::record(10, 0, 1, 0, b"\xff\xfeItems")),
                (0, support::record(15, 0, 1, 0, b"\xff\xfeOther")),
            ],
        ),
    );
    let mut definition = fixture::definition(&[("Id", 4, 0, 4), ("Memo", 11, 0, 0)], 1, 16 << 8);
    definition[88] = 12;
    definition[103] = 2;
    support::put32(&mut definition, 59, (16 << 8) | 1);
    if declared {
        definition[129..141].copy_from_slice(&[1, 0, 2, 16, 0, 0, 3, 16, 0, 0, 255, 255]);
    } else {
        support::put16(&mut definition, 129, u16::MAX);
    }
    install(bytes, 15, &definition);
    install(
        bytes,
        16,
        &page(
            0,
            &[
                (0, map(&[17])),
                (0, map(&[17])),
                (0, map(&[MEMO])),
                (0, map(&[MEMO])),
            ],
        ),
    );
    let descriptor = if external {
        single(2, MEMO, 0)
    } else {
        inline(&[65, 0])
    };
    let mut row = vec![2, 0, 1, 0, 0, 0];
    row.extend(descriptor);
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(6_u16.to_le_bytes());
    row.extend(1_u16.to_le_bytes());
    row.push(3);
    install(bytes, 17, &page(15, &[(0, row)]));
}
