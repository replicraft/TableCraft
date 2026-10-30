use crate::{
    IndexCheckStatus, NativeImage, RowInsert, TableCheck, check_table, check_table_constraints,
    read_table,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    self as support,
    insert_seed::{HISTORY, PAGE, inline, install, leaf, row},
};

pub(in crate::table::mutation::insert) fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

pub(super) struct NewRow {
    pub id: i32,
    pub binary: Option<Vec<u8>>,
}

impl NewRow {
    pub fn request(&self) -> RowInsert {
        RowInsert::new(vec![
            Value::Int32(self.id),
            self.binary
                .as_ref()
                .map_or(Value::Null, |value| Value::Binary(value.clone())),
        ])
    }
}

pub(in crate::table::mutation::insert) fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

pub(in crate::table::mutation::insert) fn checked(
    bytes: &[u8],
    rows: usize,
    aliases: usize,
) -> TableCheck {
    let sequential = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequential.rows().len(), rows);
    let check = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(check.rows(), rows);
    assert_eq!(check.indexes().len(), aliases);
    assert!(
        check
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), rows);
    assert!(constraints.relationships().is_empty());
    check
}

fn count(page: &[u8]) -> usize {
    usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()))
}
fn end(page: &[u8]) -> usize {
    let count = count(page);
    if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(
                page[14 + 2 * (count - 1)..16 + 2 * (count - 1)]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        )
    }
}

pub(super) fn expected(before: &[u8], requests: &[NewRow]) -> (Vec<u8>, Vec<u32>) {
    let mut bytes = before.to_vec();
    let mut next = before.len() / PAGE;
    let mut owned = vec![12];
    let mut hints = vec![12];
    let mut keys = vec![(1, 12 << 8), (2, (12 << 8) | 1), (3, (12 << 8) | 2)];
    let mut references = Vec::new();
    for request in requests {
        let record = row(request.id, request.binary.as_deref());
        let mut destination = hints.iter().copied().max();
        if let Some(number) = destination {
            let page = &bytes[number * PAGE..(number + 1) * PAGE];
            if count(page) >= 255 || end(page) - 14 - count(page) * 2 < record.len() + 2 {
                hints.retain(|hint| *hint != number);
                destination = None;
            }
        }
        let number = destination.unwrap_or_else(|| {
            let number = next;
            next += 1;
            bytes.resize(next * PAGE, 0);
            let mut page = support::data_page(10, &[]);
            support::put16(&mut page, 2, 4082);
            install(&mut bytes, number, &page);
            owned.push(number);
            hints.push(number);
            number
        });
        let page = &mut bytes[number * PAGE..(number + 1) * PAGE];
        let slot = count(page);
        let record_end = end(page);
        let start = record_end - record.len();
        page[start..record_end].copy_from_slice(&record);
        support::put16(page, 14 + slot * 2, start as u16);
        support::put16(page, 12, (slot + 1) as u16);
        support::put16(page, 2, (start - 16 - slot * 2) as u16);
        let reference = (number as u32) << 8 | slot as u32;
        references.push(reference);
        keys.push((request.id, reference));
    }
    keys.sort();
    assert!(keys.windows(2).all(|pair| pair[0].0 != pair[1].0));
    let root = leaf(Some(&before[13 * PAGE..14 * PAGE]), &keys);
    install(&mut bytes, 13, &root);
    support::put32(&mut bytes, 10 * PAGE + 16, 3 + requests.len() as u32);
    support::put32(&mut bytes, 10 * PAGE + 67, HISTORY + requests.len() as u32);
    bytes[11 * PAGE + 4027..12 * PAGE].copy_from_slice(&inline(owned));
    bytes[11 * PAGE + 3958..11 * PAGE + 4027].copy_from_slice(&inline(hints));
    bytes[PAGE + 4027..2 * PAGE].copy_from_slice(&inline(next..512));
    (bytes, references)
}
