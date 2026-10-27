use super::*;

fn preview(
    model: &Model,
    updates: &[Int32Update],
) -> Result<tablecraft_ace::Ace16WritePreview, EditError> {
    capture(&model.bytes).preview_ace16_split_int32_updates(
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

pub fn verify_values(model: &Model, after: &[u8], expected: &[Option<i32>]) {
    let report = check_table(
        &mut Cursor::new(after),
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
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    for (physical, descending) in model.directions.iter().copied().enumerate() {
        let keys = ordered(expected, descending);
        let expected_rows = keys
            .iter()
            .map(|key| {
                let reference = u32::from_be_bytes(key[key.len() - 4..].try_into().unwrap());
                let page = (reference >> 8) as usize;
                let slot = (reference & 255) as usize;
                let group = DATA_PAGES
                    .iter()
                    .position(|candidate| *candidate == page)
                    .unwrap();
                let row = group * ROWS_PER_PAGE + slot;
                vec![
                    Value::Int32(row as i32 + 1),
                    expected[row].map_or(Value::Null, Value::Int32),
                ]
            })
            .collect::<Vec<_>>();
        for alias in if model.aliases {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            let rows = read_table_index(
                &mut Cursor::new(after),
                CatalogScope::new(1),
                "Items",
                index_name(physical, *alias),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), expected_rows);
        }
    }
}

pub fn verify_split(
    model: &Model,
    after: &[u8],
    expected: &[Option<i32>],
    physical: usize,
) -> [usize; 2] {
    let root = ROOTS[physical];
    let base = root * PAGE_BYTES;
    assert_eq!(&after[base..base + 2], &[3, 1]);
    assert_eq!(after[base + HEIGHT], 1);
    assert_eq!(get16(after, base + PREFIX), 0);
    assert_eq!(get32(after, base + OWNER), DEFINITION as u32);
    assert_eq!(get32(after, base + PREVIOUS), 0);
    assert_eq!(get32(after, base + NEXT), 0);
    let separator = entries(after, root);
    assert_eq!(separator.len(), 1);
    let bound = &separator[0];
    let left = u32::from_be_bytes(bound[bound.len() - 4..].try_into().unwrap()) as usize;
    let right = get32(after, base + TAIL) as usize;
    assert_eq!(left, right + 1, "right EOF leaf is allocated first");
    let keys = ordered(expected, model.directions[physical]);
    let middle = keys.len() / 2;
    assert_eq!(&bound[..bound.len() - 4], keys[middle - 1]);
    assert_eq!(entries(after, left), keys[..middle]);
    assert_eq!(entries(after, right), keys[middle..]);
    for (page, previous, next) in [(left, 0, right), (right, left, 0)] {
        let start = page * PAGE_BYTES;
        assert_eq!(&after[start..start + 2], &[4, 1]);
        assert_eq!(after[start + HEIGHT], 0);
        assert_eq!(get32(after, start + OWNER), DEFINITION as u32);
        assert_eq!(get32(after, start + PREVIOUS), previous as u32);
        assert_eq!(get32(after, start + NEXT), next as u32);
        assert_eq!(get32(after, start + TAIL), 0);
    }
    [right, left]
}

pub fn verify_preservation(
    model: &Model,
    after: &[u8],
    updated_row: usize,
    new_pages: &[[usize; 2]],
) {
    let mut expected_metadata = model.bytes.clone();
    for (physical, pair) in new_pages.iter().enumerate() {
        for page in pair {
            let global = map_byte(&model.bytes, 1, *page);
            let owned = map_byte(&model.bytes, INDEX_MAPS[physical], *page);
            expected_metadata[global] &= !(1 << (*page % 8));
            expected_metadata[owned] |= 1 << (*page % 8);
        }
    }
    let data = data_offset(&model.bytes, updated_row);
    for (offset, old) in expected_metadata.iter().copied().enumerate() {
        let in_payload = (data..data + 4).contains(&offset);
        let in_index = ROOTS[..model.directions.len()].iter().any(|page| {
            let start = page * PAGE_BYTES;
            offset == start
                || (start + INDEX_FREE..start + INDEX_FREE + 2).contains(&offset)
                || (start + TAIL..start + PAGE_BYTES).contains(&offset)
        });
        if !in_payload && !in_index {
            assert_eq!(after[offset], old, "unrelated byte at {offset}");
        }
    }
    for page in ROOTS.iter().take(model.directions.len()) {
        let start = page * PAGE_BYTES;
        assert_eq!(
            &after[start + OWNER..start + TAIL],
            &model.bytes[start + OWNER..start + TAIL]
        );
    }
}

#[test]
fn literal_five_hundred_key_splits_keep_aliases_direction_and_row_ties() {
    for code in [5, 6] {
        for descending in [false, true] {
            for ties in [false, true] {
                let scores = (0..500)
                    .map(|row| Some(if ties { row / 2 + 1 } else { row + 1 }))
                    .collect();
                let model = database(code, scores, vec![descending], true, false, false);
                checked(&model);
                let output = preview(&model, &[Int32Update::new(0, 1, i32::MAX)]).unwrap();
                let after = output.candidate_bytes();
                assert_eq!(after.len(), model.bytes.len() + 2 * PAGE_BYTES);
                assert_eq!(output.changed_rows(), 1);
                let mut expected = model.scores.clone();
                expected[0] = Some(i32::MAX);
                let pair = verify_split(&model, after, &expected, 0);
                assert_eq!(pair, [ORIGINAL_PAGES, ORIGINAL_PAGES + 1]);
                if ties {
                    let left = entries(after, pair[1]);
                    let right = entries(after, pair[0]);
                    assert_eq!(&left.last().unwrap()[..5], &right[0][..5]);
                    assert_ne!(&left.last().unwrap()[5..], &right[0][5..]);
                }
                verify_values(&model, after, &expected);
                verify_preservation(&model, after, 0, &[pair]);
                assert_eq!(output.before_bytes(), model.bytes);
            }
        }
    }
}

#[test]
fn two_physical_owners_compose_all_four_new_bits_in_one_global_byte() {
    let scores = (1..=500).map(Some).collect();
    let model = database(6, scores, vec![false, true], true, false, false);
    checked(&model);
    let output = preview(&model, &[Int32Update::new(0, 1, i32::MAX)]).unwrap();
    let after = output.candidate_bytes();
    assert_eq!(after.len(), model.bytes.len() + 4 * PAGE_BYTES);
    let mut expected = model.scores.clone();
    expected[0] = Some(i32::MAX);
    let pairs = [
        verify_split(&model, after, &expected, 0),
        verify_split(&model, after, &expected, 1),
    ];
    let mut pages = pairs.into_iter().flatten().collect::<Vec<_>>();
    pages.sort();
    assert_eq!(pages, vec![24, 25, 26, 27]);
    let global = map_byte(&model.bytes, 1, 24);
    assert_eq!(model.bytes[global], 0xff);
    assert_eq!(after[global], 0xf0);
    verify_values(&model, after, &expected);
    verify_preservation(&model, after, 0, &pairs);
}

#[test]
fn nullable_peer_keys_use_existing_nonoverflowing_path_without_allocation() {
    for descending in [false, true] {
        let scores = std::iter::once(None).chain((1..300).map(Some)).collect();
        let model = database(6, scores, vec![descending], true, false, false);
        checked(&model);
        let output = preview(&model, &[Int32Update::new(1, 1, i32::MAX)]).unwrap();
        let after = output.candidate_bytes();
        assert_eq!(after.len(), model.bytes.len());
        assert_eq!(after[ROOTS[0] * PAGE_BYTES + HEIGHT], 0);
        for page in [1, DATA_USAGE, INDEX_MAPS[0]] {
            assert_eq!(
                &after[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
                &model.bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
            );
        }
        let mut expected = model.scores.clone();
        expected[1] = Some(i32::MAX);
        verify_values(&model, after, &expected);
        verify_preservation(&model, after, 1, &[]);
    }
}
