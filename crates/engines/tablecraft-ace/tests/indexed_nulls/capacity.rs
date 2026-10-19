use super::{DEFINITION, PAGE_BYTES, budget, database, preview, support};
use std::io::Cursor;
use tablecraft_ace::{EditError, FixedScalarUpdate, WriteBlocker, read_table_index};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const ROWS: usize = 600;
const ROWS_PER_PAGE: usize = 200;
const ENTRY_START: usize = 480;

fn dense() -> Vec<u8> {
    let mut bytes = database(6, false, false, true, 0);
    bytes.resize(17 * PAGE_BYTES, 0);
    support::put32(&mut bytes, DEFINITION * PAGE_BYTES + 16, ROWS as u32);
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0x90, 1])],
    ));
    for (group, page) in [12, 15, 16].into_iter().enumerate() {
        let rows = (0..ROWS_PER_PAGE)
            .map(|slot| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(((group * ROWS_PER_PAGE + slot + 1) as i32).to_le_bytes());
                row.extend(12345_i32.to_le_bytes());
                row.push(1);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(DEFINITION as u32, &rows));
    }
    let page = &mut bytes[13 * PAGE_BYTES..14 * PAGE_BYTES];
    page.fill(0xa7);
    page[..ENTRY_START].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put16(page, 2, (PAGE_BYTES - ENTRY_START - ROWS * 5) as u16);
    support::put32(page, 4, DEFINITION as u32);
    for ordinal in 0..ROWS {
        let number = [12_u32, 15, 16][ordinal / ROWS_PER_PAGE];
        let slot = ordinal % ROWS_PER_PAGE;
        let start = ENTRY_START + ordinal * 5;
        page[start] = 0;
        page[start + 1..start + 5].copy_from_slice(&((number << 8) | slot as u32).to_be_bytes());
        let end = (ordinal + 1) * 5;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    bytes
}

#[test]
fn a_fitting_growth_succeeds_but_a_required_split_rejects_the_complete_batch() {
    let bytes = dense();
    let noop = preview(
        &bytes,
        &[FixedScalarUpdate::new(0, Value::Null, Value::Null)],
    )
    .unwrap();
    assert_eq!(noop.candidate_bytes(), bytes);
    let small = preview(
        &bytes,
        &[FixedScalarUpdate::new(0, Value::Null, Value::Int32(7))],
    )
    .unwrap();
    let rows = read_table_index(
        &mut Cursor::new(small.candidate_bytes()),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), ROWS);
    assert_eq!(
        rows.rows().last().unwrap(),
        &[Value::Int32(1), Value::Int32(7)]
    );
    let updates = (0..ROWS)
        .map(|row| {
            FixedScalarUpdate::new(
                row,
                Value::Null,
                Value::Int32((row as u32).wrapping_mul(0x9e37_79b9) as i32),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        preview(&bytes, &updates).err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
}
