//! Real INDEX continuations. DATA bodies/counters remain the caller's proof;
//! these controls never publish a complete candidate with unstaged DATA bytes.
use super::*;
use tablecraft_testkit::access as support;

fn wire(key: &KeyRef) -> Vec<u8> {
    let value = i32::try_from(key.key.unwrap()).unwrap();
    [
        vec![127],
        ((value as u32) ^ 0x80000000).to_be_bytes().to_vec(),
        key.row.to_be_bytes().to_vec(),
    ]
    .concat()
}

fn frame(
    entries: &[(KeyRef, Option<u32>)],
    height: u8,
    prefix: usize,
    previous: u32,
    next: u32,
    tail: u32,
) -> Vec<u8> {
    let mut page = vec![0; 4096];
    page[..2].copy_from_slice(&[if height == 0 { 4 } else { 3 }, 1]);
    support::put32(&mut page, 4, 10);
    support::put32(&mut page, 12, previous);
    support::put32(&mut page, 16, next);
    support::put32(&mut page, 20, tail);
    support::put16(&mut page, 24, prefix as u16);
    page[26] = height;
    let mut end = 0;
    let mut first = Vec::new();
    for (position, (key, child)) in entries.iter().enumerate() {
        let mut encoded = wire(key);
        if let Some(child) = child {
            encoded.extend(child.to_be_bytes());
        }
        if position == 0 {
            first = encoded.clone();
        } else {
            assert_eq!(&encoded[..prefix], &first[..prefix]);
        }
        let suffix = &encoded[if position == 0 { 0 } else { prefix }..];
        page[480 + end..480 + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    assert!(end <= 3616);
    support::put16(&mut page, 2, (3616 - end) as u16);
    page
}

fn inline(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut row = vec![0; 69];
    for page in members {
        assert!(page < 512);
        row[5 + page / 8] |= 1 << (page % 8);
    }
    row
}

fn root_source(code: u8, aliases: bool) -> (Vec<u8>, Vec<KeyRef>) {
    let mut bytes = model::source(code, aliases, false, false).bytes;
    bytes.resize(380 * 4096, 0);
    let values = (0..361).collect::<Vec<i32>>();
    let mut keys = Vec::new();
    for (number, subset) in [(12, &values[..255]), (15, &values[255..])] {
        let records = subset
            .iter()
            .map(|value| {
                let mut row = vec![2, 0];
                row.extend(value.to_le_bytes());
                row.extend([6, 0, 6, 0, 1, 0, 1]);
                (0, row)
            })
            .collect::<Vec<_>>();
        let mut data = support::data_page(10, &records);
        support::put16(&mut data, 2, (4082 - records.len() * 15) as u16);
        bytes[number * 4096..(number + 1) * 4096].copy_from_slice(&data);
        keys.extend(subset.iter().enumerate().map(|(slot, value)| KeyRef {
            key: Some(i64::from(*value)),
            row: (number as u32) << 8 | slot as u32,
        }));
    }
    for (position, key) in keys.iter().enumerate() {
        let number = 17 + position;
        let previous = if position == 0 { 0 } else { number - 1 };
        let next = if position == 360 { 0 } else { number + 1 };
        bytes[number * 4096..(number + 1) * 4096].copy_from_slice(&frame(
            &[(*key, None)],
            0,
            0,
            previous as u32,
            next as u32,
            0,
        ));
    }
    let bounds = keys
        .iter()
        .enumerate()
        .map(|(position, key)| (*key, Some((17 + position) as u32)))
        .collect::<Vec<_>>();
    bytes[13 * 4096..14 * 4096].copy_from_slice(&frame(&bounds, 1, 3, 0, 0, 0));
    bytes[11 * 4096..12 * 4096].copy_from_slice(&support::data_page(
        0,
        &[(0, inline([12, 15])), (0, inline([15]))],
    ));
    bytes[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(
        0,
        &[(0, inline(std::iter::once(13).chain(17..378)))],
    ));
    bytes[4096..2 * 4096].copy_from_slice(&support::data_page(1, &[(0, inline(380..512))]));
    support::put32(&mut bytes, 10 * 4096 + 16, 361);
    (bytes, keys)
}

#[test]
fn prefix_loss_grows_original_root_into_two_fresh_internal_siblings() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        for aliases in [false, true] {
            let (before, mut keys) = root_source(code, aliases);
            let check = crate::check_table(
                &mut Cursor::new(&before),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(check.rows(), 361);
            assert!(
                check
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let image = capture(&before);
            keys.push(KeyRef {
                key: Some(i64::from(i32::MAX)),
                row: (15 << 8) | 106,
            });
            let plan = continue_index::<_, ENTRY_BUFFER>(
                operation(&image, &check, &keys, &profile),
                State::new(&image, 10),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                plan.added
                    .iter()
                    .map(|page| page.number)
                    .collect::<Vec<_>>(),
                [380, 381]
            );
            assert_eq!(plan.append.indexes().count(), 2);
            let actual = compose(&before, &plan);
            let mut expected = before.clone();
            expected.resize(382 * 4096, 0);
            let last = frame(&[(keys[360], None), (keys[361], None)], 0, 0, 376, 0, 0);
            // The old one-entry leaf had no nonzero suffix, so its exact old
            // residue and a literal two-entry frame agree outside active data.
            expected[377 * 4096..378 * 4096].copy_from_slice(&last);
            let left_bounds = (0..179)
                .map(|position| (keys[position], Some((17 + position) as u32)))
                .collect::<Vec<_>>();
            let right_bounds = (180..360)
                .map(|position| (keys[position], Some((17 + position) as u32)))
                .collect::<Vec<_>>();
            expected[381 * 4096..382 * 4096].copy_from_slice(&frame(
                &left_bounds,
                1,
                0,
                0,
                380,
                196,
            ));
            expected[380 * 4096..381 * 4096].copy_from_slice(&frame(
                &right_bounds,
                1,
                0,
                381,
                0,
                377,
            ));
            let root = frame(
                &[(keys[179], Some(381)), (keys[361], Some(380))],
                2,
                0,
                0,
                0,
                0,
            );
            let active_end = 480 + 26;
            let mut retained_root = before[13 * 4096..14 * 4096].to_vec();
            retained_root[..active_end].copy_from_slice(&root[..active_end]);
            expected[13 * 4096..14 * 4096].copy_from_slice(&retained_root);
            for number in [380, 381] {
                expected[4096 + 4027 + 5 + number / 8] &= !(1 << (number % 8));
                expected[14 * 4096 + 4027 + 5 + number / 8] |= 1 << (number % 8);
            }
            model::assert_image(&actual, &expected);
        }
    }
}

#[test]
fn root_leaf_bulk_continuation_normalizes_beyond_the_legacy_two_halves() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let mut before = super::routing::source(6, true);
    let original = [
        KeyRef {
            key: Some(10),
            row: 12 << 8,
        },
        KeyRef {
            key: Some(20),
            row: (12 << 8) | 1,
        },
        KeyRef {
            key: Some(30),
            row: (12 << 8) | 2,
        },
    ];
    let entries = original
        .iter()
        .copied()
        .map(|key| (key, None))
        .collect::<Vec<_>>();
    before[13 * 4096..14 * 4096].copy_from_slice(&frame(&entries, 0, 4, 0, 0, 0));
    before[14 * 4096..15 * 4096].copy_from_slice(&support::data_page(0, &[(0, inline([13]))]));
    let check = super::routing::checked(&before);
    let image = capture(&before);
    let mut append = State::new(&image, 10);
    let mut input = image.reader();
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    for number in 19..27 {
        append
            .reserve_data(number, reader.budget, reader.cancelled)
            .unwrap();
        for (reference, role, set) in [
            (11 << 8, MapRole::Owned, true),
            ((11 << 8) | 1, MapRole::TableFreeSpace, true),
            (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree, false),
        ] {
            let map = RepresentedMap::read(&mut reader, reference, role).unwrap();
            let cell = map.future_cell(&mut reader, number).unwrap();
            append::claim(&mut append.map_edits, cell, set, &mut reader).unwrap();
        }
    }
    let mut keys = original.to_vec();
    keys.extend((0..2000).map(|position| KeyRef {
        key: Some(i64::from(i32::MIN + position * 65536)),
        row: ((19 + position as u32 / 250) << 8) | (position as u32 % 250),
    }));
    keys.sort_by_key(|key| (key.key, key.row));
    let plan = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &check,
            index: check.indexes()[0].index(),
            profile: &profile,
            final_keys: &keys,
        },
        append,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(plan.append.indexes().count() > 2);
    assert_eq!(
        plan.append.reservations_len(),
        8 + plan.append.indexes().count()
    );
    let actual = compose(&before, &plan);
    assert_eq!(actual[13 * 4096], 3);
    assert_eq!(actual[13 * 4096 + 26], 1);
    let tail = u32::from_le_bytes(actual[13 * 4096 + 20..13 * 4096 + 24].try_into().unwrap());
    assert_eq!(tail, 27);
    assert!(plan.added.iter().all(|page| page.bytes[..2] == [4, 1]));
    assert_eq!(image.as_bytes(), before);
}
