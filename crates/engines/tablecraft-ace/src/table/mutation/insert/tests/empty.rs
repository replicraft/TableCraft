//! First visible row from a genuinely empty table and primary leaf.
use super::model;
use crate::{RowInsert, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::insert_seed as source_model;

const PAGE: usize = 4096;
const DEFINITION: usize = 10 * PAGE;
const DATA: usize = 12 * PAGE;
const PRIMARY: usize = 13 * PAGE;

fn empty_source(code: u8, alias: bool) -> Vec<u8> {
    let mut bytes = source_model::source(code, alias, false, None);
    bytes[DEFINITION + 16..DEFINITION + 20].copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        &bytes[DEFINITION + 67..DEFINITION + 71],
        &17_u32.to_le_bytes()
    );

    // This is a new literal empty DATA page, not a live-row count relabel.
    bytes[DATA..DATA + PAGE].fill(0);
    bytes[DATA..DATA + 2].copy_from_slice(&[1, 1]);
    bytes[DATA + 2..DATA + 4].copy_from_slice(&4082_u16.to_le_bytes());
    bytes[DATA + 4..DATA + 8].copy_from_slice(&10_u32.to_le_bytes());

    // No active entry endpoints remain. Preserve the former body as residue.
    bytes[PRIMARY + 2..PRIMARY + 4].copy_from_slice(&3616_u16.to_le_bytes());
    bytes[PRIMARY + 24..PRIMARY + 26].copy_from_slice(&0_u16.to_le_bytes());
    bytes[PRIMARY + 27..PRIMARY + 480].fill(0);
    bytes
}

fn first_image(before: &[u8], binary_present: bool) -> Vec<u8> {
    let mut expected = before.to_vec();
    let row = [
        2,
        0,
        0,
        0,
        0,
        128,
        6,
        0,
        6,
        0,
        1,
        0,
        if binary_present { 3 } else { 1 },
    ];
    expected[DATA + 4083..DATA + 4096].copy_from_slice(&row);
    expected[DATA + 12..DATA + 14].copy_from_slice(&1_u16.to_le_bytes());
    expected[DATA + 14..DATA + 16].copy_from_slice(&4083_u16.to_le_bytes());
    expected[DATA + 2..DATA + 4].copy_from_slice(&4067_u16.to_le_bytes());

    // Ascending non-NULL Int32 MIN has zero sign-biased payload. The final
    // four bytes are the original logical ROW: page 12, visible slot zero.
    let entry = [127, 0, 0, 0, 0, 0, 0, 12, 0];
    expected[PRIMARY + 480..PRIMARY + 489].copy_from_slice(&entry);
    expected[PRIMARY + 28] = 2;
    expected[PRIMARY + 2..PRIMARY + 4].copy_from_slice(&3607_u16.to_le_bytes());
    expected[DEFINITION + 16..DEFINITION + 20].copy_from_slice(&1_u32.to_le_bytes());
    expected[DEFINITION + 67..DEFINITION + 71].copy_from_slice(&18_u32.to_le_bytes());
    assert_eq!(
        &expected[PRIMARY + 489..PRIMARY + PAGE],
        &before[PRIMARY + 489..PRIMARY + PAGE]
    );
    assert_eq!(expected.len(), before.len());
    expected
}

#[test]
fn empty_primary_accepts_first_visible_minimum_key_with_null_or_empty_binary() {
    for code in [5, 6] {
        for alias in [false, true] {
            for binary_present in [false, true] {
                let before = empty_source(code, alias);
                let aliases = if alias { 2 } else { 1 };
                model::checked(&before, 0, aliases);
                let expected = first_image(&before, binary_present);
                model::checked(&expected, 1, aliases);
                let binary = if binary_present {
                    Value::Binary(Vec::new())
                } else {
                    Value::Null
                };
                let wanted = vec![Value::Int32(i32::MIN), binary];
                let expected_rows = read_table(
                    &mut Cursor::new(&expected),
                    CatalogScope::new(1),
                    "Items",
                    &mut model::budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(expected_rows.rows(), std::slice::from_ref(&wanted));

                let candidate = model::capture(&before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &[RowInsert::new(wanted)],
                        &mut model::budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(candidate.changed_rows(), 1);
                assert_eq!(candidate.candidate_bytes(), expected);
                model::checked(candidate.candidate_bytes(), 1, aliases);
            }
        }
    }
}
