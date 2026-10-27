use super::*;

pub const OTHER_DEFINITION: usize = 15;
pub const OTHER_MAP: usize = 16;
pub const OTHER_DATA: usize = 17;
pub const GLOBAL_BITMAP: usize = 18;
pub const OTHER_HINT_BITMAP: usize = 19;
pub const INDEX_ROOT: usize = 20;
pub const OTHER_DECLARATIONS: usize = 129;
pub const OWNED_REFERENCE: u32 = (11 << 8) | 2;
pub const HINT_REFERENCE: u32 = (11 << 8) | 3;
pub const OTHER_OWNED: u32 = (16 << 8) | 2;
pub const OTHER_HINT: u32 = (16 << 8) | 3;
pub const HISTORY_HINT: u32 = (16 << 8) | 5;
pub const TABLE_USAGE: usize = 55;
pub const TABLE_HINT: usize = 59;
const FIELD_BYTES: usize = 25;
const PHYSICAL_COUNTER_BYTES: usize = 12;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const PHYSICAL_START: usize = 141;
const LOGICAL_START: usize = PHYSICAL_START + PHYSICAL_BYTES;
const INDEX_DATA: usize = 480;
const INDEX_BITMAP: usize = 27;
const BITMAP_DATA: usize = 4;

pub fn fresh(code: u8) -> Vec<u8> {
    let mut bytes = source(code, 1);
    set_maps(&mut bytes, &[], &[], &[OLE]);
    let memo = inline(&[65, 0]);
    let ole = single(1, OLE, 0);
    install(
        &mut bytes,
        ROOT,
        &page(10, &[(0, record(1, Some(&memo), Some(&ole)))]),
    );
    other_table_alias(&mut bytes, true, false);
    other_maps(&mut bytes, &[OTHER_DATA], &[OTHER_DATA], &[]);
    bytes
}

pub fn other_maps(bytes: &mut [u8], owned: &[usize], hints: &[usize], index_owned: &[usize]) {
    install(
        bytes,
        OTHER_MAP,
        &page(
            0,
            &[
                (0, map(owned)),
                (0, map(hints)),
                (0, map(&[])),
                (0, map(&[])),
                (0, map(index_owned)),
                (0, map(&[])),
            ],
        ),
    );
}

pub fn bind_other(bytes: &mut [u8], owned: u32, hint: u32) {
    let offset = OTHER_DEFINITION * PAGE + OTHER_DECLARATIONS;
    support::put32(bytes, offset + 2, owned);
    support::put32(bytes, offset + 6, hint);
}

pub fn history(bytes: &mut [u8], owned: u32, hint: u32) {
    let offset = OTHER_DEFINITION * PAGE + OTHER_DECLARATIONS + 10;
    support::put16(bytes, offset, 99);
    support::put32(bytes, offset + 2, owned);
    support::put32(bytes, offset + 6, hint);
    support::put16(bytes, offset + 10, u16::MAX);
}

pub fn healthy(bytes: &[u8], other_rows: usize) {
    assert_eq!(read(bytes)[0][1], Value::Text("A".into()));
    for name in ["Items", "Other"] {
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let storage = check_table_long_values(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(storage.rows(), if name == "Items" { 1 } else { other_rows });
        assert_eq!(storage.columns()[0].status(), LongValueCheckStatus::Checked);
        assert_eq!(storage.columns()[0].owned_pages(), 0);
        let rows = read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows().len(),
            if name == "Items" { 1 } else { other_rows }
        );
    }
}

pub fn reject_alias(bytes: &[u8]) {
    for updates in [
        vec![],
        vec![memo_update(Value::Text("A".into()))],
        vec![memo_update(Value::Text("Ā".repeat(33)))],
    ] {
        assert_eq!(
            run(bytes, "Memo", &updates).err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

pub fn indirect_global_and_other_hint(bytes: &mut [u8], shared: bool) {
    let mut global = vec![0; PAGE];
    global[..2].copy_from_slice(&[5, 1]);
    for number in EOF..(PAGE - BITMAP_DATA) * 8 {
        global[BITMAP_DATA + number / 8] |= 1 << (number % 8);
    }
    install(bytes, GLOBAL_BITMAP, &global);
    let mut empty = vec![0; PAGE];
    empty[..2].copy_from_slice(&[5, 1]);
    install(bytes, OTHER_HINT_BITMAP, &empty);
    let mut global_reference = vec![1];
    global_reference.extend((GLOBAL_BITMAP as u32).to_le_bytes());
    install(bytes, 1, &page(1, &[(0, global_reference)]));
    let bitmap = if shared {
        GLOBAL_BITMAP
    } else {
        OTHER_HINT_BITMAP
    };
    let mut hint = vec![1];
    hint.extend((bitmap as u32).to_le_bytes());
    install(
        bytes,
        OTHER_MAP,
        &page(
            0,
            &[
                (0, map(&[OTHER_DATA])),
                (0, hint),
                (0, map(&[])),
                (0, map(&[])),
                (0, map(&[])),
                (0, map(&[])),
            ],
        ),
    );
}

pub fn indexed_other(bytes: &mut [u8]) {
    let original = bytes[OTHER_DEFINITION * PAGE..(OTHER_DEFINITION + 1) * PAGE].to_vec();
    let mut definition = vec![0; PAGE];
    definition[..63].copy_from_slice(&original[..63]);
    definition[63 + PHYSICAL_COUNTER_BYTES..PHYSICAL_START]
        .copy_from_slice(&original[63..OTHER_DECLARATIONS]);
    assert_eq!(
        PHYSICAL_START,
        63 + PHYSICAL_COUNTER_BYTES + 2 * FIELD_BYTES + 16
    );
    support::put32(&mut definition, 47, 2);
    support::put32(&mut definition, 51, 1);
    support::put32(&mut definition, 67, 1);
    for position in 0..10 {
        support::put16(&mut definition, PHYSICAL_START + 4 + position * 3, u16::MAX);
    }
    support::put16(&mut definition, PHYSICAL_START + 4, 0);
    definition[PHYSICAL_START + 6] = 1;
    support::put32(&mut definition, PHYSICAL_START + 34, (16 << 8) | 4);
    support::put32(&mut definition, PHYSICAL_START + 38, INDEX_ROOT as u32);
    definition[PHYSICAL_START + 46] = 0x80;
    support::put32(&mut definition, LOGICAL_START + LOGICAL_BYTES + 4, 1);
    let mut position = LOGICAL_START + 2 * LOGICAL_BYTES;
    for name in ["IdKey", "IdTwin"] {
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        support::put16(&mut definition, position, encoded.len() as u16);
        position += 2;
        definition[position..position + encoded.len()].copy_from_slice(&encoded);
        position += encoded.len();
    }
    assert_eq!(position, 275);
    support::put16(&mut definition, position, 1);
    support::put32(&mut definition, position + 2, OTHER_OWNED);
    support::put32(&mut definition, position + 6, OTHER_HINT);
    support::put16(&mut definition, position + 10, u16::MAX);
    install(bytes, OTHER_DEFINITION, &definition);
    other_maps(bytes, &[OTHER_DATA], &[OTHER_DATA], &[INDEX_ROOT]);
    let mut root = vec![0; PAGE];
    root[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut root, 4, OTHER_DEFINITION as u32);
    let mut entry = vec![127, 128, 0, 0, 1];
    entry.extend(((OTHER_DATA as u32) << 8).to_be_bytes());
    root[INDEX_DATA..INDEX_DATA + entry.len()].copy_from_slice(&entry);
    root[INDEX_BITMAP + entry.len() / 8] |= 1 << (entry.len() % 8);
    support::put16(&mut root, 2, (PAGE - INDEX_DATA - entry.len()) as u16);
    install(bytes, INDEX_ROOT, &root);
}
