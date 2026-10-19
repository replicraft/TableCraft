use super::*;
use tablecraft_ace::WriteBlocker;

const ROWS: usize = 600;
const ROWS_PER_PAGE: usize = 200;
const ENTRY_START: usize = 480;

fn dense() -> Vec<u8> {
    let mut bytes = model::with_alias(
        numeric_fixture::database(
            6,
            &[4, 4],
            &[(0, false), (1, false)],
            &[vec![Some(0), None]],
            0,
        ),
        2,
    );
    bytes.resize(17 * PAGE_BYTES, 0);
    support::put32(&mut bytes, 10 * PAGE_BYTES + 16, ROWS as u32);
    bytes[11 * PAGE_BYTES..12 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0x90, 1])],
    ));
    for number in [12, 15, 16] {
        let rows = (0..ROWS_PER_PAGE)
            .map(|_| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(0_i32.to_le_bytes());
                row.extend(12345_i32.to_le_bytes());
                row.push(1);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[number * PAGE_BYTES..(number + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(10, &rows));
    }
    // Constant first component and NULL second component: prefix six leaves
    // exact four-byte row references after the first full ten-byte entry.
    let page = &mut bytes[14 * PAGE_BYTES..15 * PAGE_BYTES];
    page.fill(0xa7);
    page[..ENTRY_START].fill(0);
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(page, 4, 10);
    support::put16(page, 24, 6);
    let mut end = 0;
    for ordinal in 0..ROWS {
        let number = [12_u32, 15, 16][ordinal / ROWS_PER_PAGE];
        let slot = ordinal % ROWS_PER_PAGE;
        if ordinal == 0 {
            page[ENTRY_START..ENTRY_START + 6].copy_from_slice(&[127, 128, 0, 0, 0, 0]);
            end = 6;
        }
        page[ENTRY_START + end..ENTRY_START + end + 4]
            .copy_from_slice(&((number << 8) | slot as u32).to_be_bytes());
        end += 4;
        page[27 + end / 8] |= 1 << (end % 8);
    }
    support::put16(page, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    bytes
}

#[test]
fn tuple_growth_fits_at_eight_free_bytes_or_rejects_a_required_split_atomically() {
    let bytes = dense();
    let run = |spread| {
        let updates = (0..ROWS)
            .map(|row| {
                FixedScalarUpdate::new(
                    row,
                    Value::Null,
                    Value::Int32(if spread {
                        (row as u32).wrapping_mul(0x9e37_79b9) as i32
                    } else {
                        row as i32
                    }),
                )
            })
            .collect::<Vec<_>>();
        capture(&bytes).preview_ace16_fixed_updates(
            "Items",
            "Score",
            &updates,
            &mut ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000),
            &|| false,
        )
    };
    let candidate = run(false).unwrap();
    assert_eq!(candidate.changed_rows(), ROWS);
    assert_eq!(candidate.candidate_bytes().len(), bytes.len());
    assert_eq!(
        u16::from_le_bytes(
            candidate.candidate_bytes()[14 * PAGE_BYTES + 2..14 * PAGE_BYTES + 4]
                .try_into()
                .unwrap()
        ),
        8
    );
    for index in ["Compound", "Alias"] {
        let table = read_table_index(
            &mut Cursor::new(candidate.candidate_bytes()),
            CatalogScope::new(1),
            "Items",
            index,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows(),
            &(0..ROWS)
                .map(|row| vec![Value::Int32(0), Value::Int32(row as i32)])
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        run(true).err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
}
