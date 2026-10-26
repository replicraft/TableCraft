//! Independent synthetic byte/model checks; no native Access qualification.
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, IndexCheckStatus, NativeImage, RowDelete, check_table,
    read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

pub fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
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

pub fn ace(mut bytes: Vec<u8>, code: u8) -> Vec<u8> {
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    bytes
}

pub fn request(row: usize, nullable: bool) -> RowDelete {
    let id = row as i32 + 1;
    let score = if nullable && row == 0 { 0 } else { id * 10 };
    let mut expected = vec![2, 0];
    expected.extend(id.to_le_bytes());
    expected.extend(score.to_le_bytes());
    expected.push(if nullable && row == 0 { 1 } else { 3 });
    RowDelete::new(row, expected)
}

pub fn preview(bytes: &[u8], deletes: &[RowDelete]) -> Result<Ace16DeletePreview, EditError> {
    capture(bytes).preview_ace16_deletes("Items", deletes, &mut budget(), &|| false)
}

pub fn table_rows(bytes: &[u8]) -> Vec<Vec<Value>> {
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

pub fn index_ids(bytes: &[u8], name: &str) -> Vec<i32> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        name,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .iter()
    .map(|row| match row[0] {
        Value::Int32(id) => id,
        _ => panic!("independent fixture Id must remain Int32"),
    })
    .collect()
}

pub fn assert_checked(bytes: &[u8]) {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| { index.status() == IndexCheckStatus::Checked })
    );
}

pub fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

pub fn count(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub fn assert_preserved(
    before: &[u8],
    candidate: &Ace16DeletePreview,
    deleted: &[usize],
    index_page: Option<usize>,
) {
    let after = candidate.candidate_bytes();
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.deleted_rows(), deleted.len());
    assert_eq!(after.len(), before.len());
    assert_eq!(count(after, 10 * 4096 + 16), 3 - deleted.len() as u32);
    assert_eq!(
        word(after, 12 * 4096 + 12),
        3,
        "physical slot count is historical"
    );

    let mut allowed = vec![false; before.len()];
    allowed[10 * 4096 + 16..10 * 4096 + 20].fill(true);
    for row in 0..3 {
        let offset = 12 * 4096 + 14 + row * 2;
        let original = word(before, offset);
        let expected = if deleted.contains(&row) {
            original | 0xc000
        } else {
            original
        };
        assert_eq!(word(after, offset), expected, "slot {row}");
        if deleted.contains(&row) {
            allowed[offset..offset + 2].fill(true);
        }
    }
    if let Some(page) = index_page {
        let offset = page * 4096;
        allowed[offset + 2..offset + 4].fill(true);
        allowed[offset + 24..offset + 26].fill(true);
        allowed[offset + 27..offset + 4096].fill(true);
    }
    for (offset, (&before, &after)) in before.iter().zip(after).enumerate() {
        assert!(
            allowed[offset] || before == after,
            "unapproved changed byte {offset}"
        );
    }
    assert_checked(after);
}
