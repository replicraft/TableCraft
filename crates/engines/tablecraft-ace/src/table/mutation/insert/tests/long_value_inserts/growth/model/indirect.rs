//! A declared indirect row gains its missing bitmap without map initialization.
use super::*;
use crate::RowInsert;

fn source_and_expected(code: u8) -> Case {
    let mut case = source(code, 600, true);
    let mut owned = vec![0; 69];
    owned[0] = 1;
    map_row(&mut case.before, 2, &owned);
    let value = (0..65)
        .map(|position| (position * 73 + 19) as u8)
        .collect::<Vec<_>>();
    let mut descriptor = vec![65, 0, 0, 64];
    descriptor.extend((600_u32 << 8).to_le_bytes());
    descriptor.extend([0; 4]);
    let mut expected = frames::expected(
        &case,
        &[frames::Row {
            id: 10,
            group: Some(100),
            binary: Some(descriptor),
        }],
    );
    expected[PAGE..2 * PAGE].copy_from_slice(&case.before[PAGE..2 * PAGE]);
    expected[11 * PAGE..12 * PAGE].copy_from_slice(&case.before[11 * PAGE..12 * PAGE]);
    expected.resize(602 * PAGE, 0);
    expected[600 * PAGE..601 * PAGE].copy_from_slice(&lval_page(&value));
    let mut bitmap = vec![0; PAGE];
    bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
    bitmap[79] = 1; // Page600; metadata601 remains clear in this owned map.
    expected[601 * PAGE..602 * PAGE].copy_from_slice(&bitmap);
    owned[1..5].copy_from_slice(&601_u32.to_le_bytes());
    map_row(&mut expected, 2, &owned);
    map_row(&mut expected, 3, &inline_map(600, &[600]));
    expected[17 * PAGE + 79] &= !3;
    assert_eq!(
        &expected[16 * PAGE..17 * PAGE],
        &case.before[16 * PAGE..17 * PAGE]
    );
    Case {
        before: case.before,
        expected,
        value,
        bodies: vec![600],
        bitmaps: vec![601],
        empty: true,
    }
}

#[test]
fn existing_indirect_owned_row_fills_pointer_zero_and_excludes_bitmap_membership() {
    for code in [5, 6] {
        let case = source_and_expected(code);
        checked(&case.before, &rows(None));
        checked(&case.expected, &rows(Some(&case.value)));
        let owned = row_start(&case.before, 11, 2);
        assert_eq!(&case.before[owned..owned + 5], &[1, 0, 0, 0, 0]);
        assert_eq!(&case.expected[owned..owned + 5], &[1, 89, 2, 0, 0]);
        assert_eq!(&case.expected[601 * PAGE..601 * PAGE + 4], &[5, 1, 0, 0]);
        assert_eq!(case.expected[601 * PAGE + 79], 1);
        assert_eq!(case.expected[601 * PAGE + 79] & 2, 0);
        let requests = [RowInsert::new(vec![
            Value::Int32(10),
            Value::Int32(100),
            Value::Binary(case.value.clone()),
        ])];
        let candidate = original::capture(&case.before)
            .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.candidate_bytes(), case.expected);
        checked(candidate.candidate_bytes(), &rows(Some(&case.value)));
    }
}
