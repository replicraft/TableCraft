//! Literal source pressure and healthy-semantic adversaries for child growth.
use crate::{
    EditError, IndexCheckStatus, NativeImage, RowInsert, WriteBlocker, check_table,
    check_table_constraints,
    table::{
        indexes::mutation::IndexDataPatch,
        mutation::insert::{audit, compose, finish, preflight, prepare, tests::model},
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const TABLE: usize = 10;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn wire(value: i32, reference: u32) -> [u8; 9] {
    let mut bytes = [0; 9];
    bytes[0] = 127;
    bytes[1..5].copy_from_slice(&((value as u32) ^ 0x80000000).to_be_bytes());
    bytes[5..].copy_from_slice(&reference.to_be_bytes());
    bytes
}

fn inline(members: &[usize]) -> Vec<u8> {
    let mut row = vec![0; 69];
    for member in members {
        row[5 + member / 8] |= 1 << (member % 8);
    }
    row
}

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn leaves(root: usize) -> (usize, usize) {
    if root == 13 { (17, 18) } else { (100, 101) }
}

fn leaf(entries: &[[u8; 9]], prefix: usize, previous: usize, next: usize, residue: u8) -> Vec<u8> {
    let mut bytes = vec![residue; PAGE];
    bytes[..480].fill(0);
    bytes[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut bytes, 4, TABLE as u32);
    support::put32(&mut bytes, 12, previous as u32);
    support::put32(&mut bytes, 16, next as u32);
    support::put16(&mut bytes, 24, prefix as u16);

    let mut active = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        bytes[480 + active..480 + active + suffix.len()].copy_from_slice(suffix);
        active += suffix.len();
        bytes[27 + active / 8] |= 1 << (active % 8);
    }
    assert!(active <= 3616);
    support::put16(&mut bytes, 2, (3616 - active) as u16);
    bytes
}

fn source(
    code: u8,
    left: &[i32],
    right: &[i32],
    eof: usize,
    root: usize,
    empty_hint: bool,
) -> Vec<u8> {
    assert!(left.last().unwrap() < right.first().unwrap());
    let (left_page, right_page) = leaves(root);
    let mut bytes = model::source(code, true, false, None);
    bytes.resize(eof * PAGE, 0);
    bytes[13 * PAGE..14 * PAGE].fill(0);
    support::put32(
        &mut bytes,
        TABLE * PAGE + 16,
        (left.len() + right.len()) as u32,
    );
    support::put32(
        &mut bytes,
        TABLE * PAGE + 67,
        (left.len() + right.len() + 248) as u32,
    );
    support::put32(&mut bytes, TABLE * PAGE + 181, root as u32);

    let values = left.iter().chain(right).copied().collect::<Vec<_>>();
    let mut keys = Vec::new();
    let mut data_pages = Vec::new();
    for (position, subset) in values.chunks(255).enumerate() {
        let number = if position == 0 { 12 } else { 14 + position };
        assert!(![root, left_page, right_page].contains(&number));
        let records = subset
            .iter()
            .map(|value| {
                let mut row = vec![2, 0];
                row.extend(value.to_le_bytes());
                row.extend([6, 0, 6, 0, 1, 0, 1]);
                (0, row)
            })
            .collect::<Vec<_>>();
        let mut page = support::data_page(TABLE as u32, &records);
        support::put16(&mut page, 2, (4082 - records.len() * 15) as u16);
        install(&mut bytes, number, &page);
        data_pages.push(number);
        for (slot, value) in subset.iter().enumerate() {
            keys.push(wire(*value, ((number as u32) << 8) | slot as u32));
        }
    }
    let hint = if empty_hint {
        Vec::new()
    } else {
        vec![*data_pages.last().unwrap()]
    };
    install(
        &mut bytes,
        11,
        &support::data_page(0, &[(0, inline(&data_pages)), (0, inline(&hint))]),
    );
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline(&[root, left_page, right_page]))]),
    );
    install(
        &mut bytes,
        left_page,
        &leaf(&keys[..left.len()], 1, 0, right_page, 0xb6),
    );
    install(
        &mut bytes,
        right_page,
        &leaf(&keys[left.len()..], 1, left_page, 0, 0xc7),
    );

    let mut root_bytes = vec![0xa7; PAGE];
    root_bytes[..480].fill(0);
    root_bytes[..2].copy_from_slice(&[3, 1]);
    support::put16(&mut root_bytes, 2, 3603);
    support::put32(&mut root_bytes, 4, TABLE as u32);
    support::put32(&mut root_bytes, 20, right_page as u32);
    root_bytes[26] = 1;
    root_bytes[480..489].copy_from_slice(&keys[left.len() - 1]);
    root_bytes[489..493].copy_from_slice(&(left_page as u32).to_be_bytes());
    root_bytes[28] |= 1 << 5;
    install(&mut bytes, root, &root_bytes);

    if eof <= 512 {
        let members = (eof..512).collect::<Vec<_>>();
        install(
            &mut bytes,
            1,
            &support::data_page(1, &[(0, inline(&members))]),
        );
    } else {
        let number = eof - 1;
        let mut bitmap = vec![0; PAGE];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        for page in eof..32736 {
            bitmap[4 + page / 8] |= 1 << (page % 8);
        }
        install(&mut bytes, number, &bitmap);
        let mut row = vec![0; 69];
        row[0] = 1;
        support::put32(&mut row, 1, number as u32);
        install(&mut bytes, 1, &support::data_page(1, &[(0, row)]));
    }
    bytes
}

fn pressured(code: u8, target_left: bool, eof: usize, root: usize, empty_hint: bool) -> Vec<u8> {
    let (left, right) = if target_left {
        ((-226..0).chain(1..=225).collect::<Vec<_>>(), vec![226])
    } else {
        (vec![-1000], (-225..0).chain(1..=226).collect::<Vec<_>>())
    };
    source(code, &left, &right, eof, root, empty_hint)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn checked(bytes: &[u8], rows: usize) {
    let structural = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(structural.rows(), rows);
    assert_eq!(structural.indexes().len(), 2);
    assert!(
        structural
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
}

fn requests(values: &[i32]) -> Vec<RowInsert> {
    values
        .iter()
        .map(|value| RowInsert::new(vec![Value::Int32(*value), Value::Null]))
        .collect()
}

fn word(bytes: &[u8], number: usize, offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[number * PAGE + offset..number * PAGE + offset + 4]
            .try_into()
            .unwrap(),
    )
}

#[test]
fn independent_child_growth_covers_both_sides_peer_bulk_and_qualified_map_transitions() {
    for code in [5, 6] {
        for target_left in [false, true] {
            for peer_bulk in [false, true] {
                for (eof, root, promoted) in [(19, 13, false), (598, 95, false), (599, 95, true)] {
                    let before = pressured(code, target_left, eof, root, false);
                    checked(&before, 452);
                    let image = capture(&before);
                    let mut values = vec![0];
                    if peer_bulk {
                        values.push(if target_left { 227 } else { -1001 });
                    }
                    let requests = requests(&values);
                    let structural =
                        preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
                    let staged =
                        prepare(&image, &structural, &requests, &mut budget(), &|| false).unwrap();
                    let candidate =
                        finish(image, "Items", &requests, &staged, &mut budget(), &|| false)
                            .unwrap();
                    let after = candidate.candidate_bytes();
                    checked(after, 452 + values.len());

                    let (left, right) = leaves(root);
                    let retained = if target_left { left } else { right };
                    assert_eq!(after.len(), (eof + 1 + usize::from(promoted)) * PAGE);
                    assert_eq!(staged.indexes.added.len(), 1 + usize::from(promoted));
                    assert_eq!(staged.indexes.promotions.len(), usize::from(promoted));
                    assert_eq!(&after[eof * PAGE..eof * PAGE + 2], &[4, 1]);
                    assert_eq!(
                        u16::from_le_bytes(
                            after[eof * PAGE + 24..eof * PAGE + 26].try_into().unwrap()
                        ),
                        0
                    );
                    assert!(
                        after[eof * PAGE + 2514..(eof + 1) * PAGE]
                            .iter()
                            .all(|byte| *byte == 0)
                    );
                    assert_eq!(word(after, root, 20), right as u32);
                    assert_eq!(word(after, retained, 12), eof as u32);
                    assert_eq!(
                        word(after, eof, 12),
                        if target_left { 0 } else { left as u32 }
                    );
                    assert_eq!(word(after, eof, 16), retained as u32);
                    assert_eq!(
                        word(after, left, 16),
                        if target_left {
                            right as u32
                        } else {
                            eof as u32
                        }
                    );
                    assert_eq!(
                        word(after, right, 12),
                        if target_left { left as u32 } else { eof as u32 }
                    );
                    assert_eq!(
                        &after[root * PAGE + 506..(root + 1) * PAGE],
                        &before[root * PAGE + 506..(root + 1) * PAGE]
                    );
                    assert_eq!(word(after, TABLE, 16), (452 + values.len()) as u32);
                    assert_eq!(word(after, TABLE, 67), (700 + values.len()) as u32);
                }
            }
        }
    }
}

#[test]
fn odd_halves_and_data_first_share_the_cursor_without_repartitioning_original_bounds() {
    for target_left in [false, true] {
        for empty_hint in [false, true] {
            let before = pressured(5, target_left, 19, 13, empty_hint);
            checked(&before, 452);
            let image = capture(&before);
            let values = [0, if target_left { -227 } else { 227 }];
            let requests = requests(&values);
            let structural =
                preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
            let staged = prepare(&image, &structural, &requests, &mut budget(), &|| false).unwrap();
            let candidate =
                finish(image, "Items", &requests, &staged, &mut budget(), &|| false).unwrap();
            let after = candidate.candidate_bytes();
            checked(after, 454);

            let new_left = 19 + usize::from(empty_hint);
            let retained = if target_left { 17 } else { 18 };
            assert_eq!(after.len(), (new_left + 1) * PAGE);
            assert_eq!(staged.indexes.added.len(), 1);
            assert_eq!(staged.indexes.added[0].number as usize, new_left);
            assert_eq!(&after[new_left * PAGE..new_left * PAGE + 2], &[4, 1]);
            assert_eq!(
                u16::from_le_bytes(
                    after[new_left * PAGE + 2..new_left * PAGE + 4]
                        .try_into()
                        .unwrap()
                ),
                1582
            );
            assert_eq!(
                u16::from_le_bytes(
                    after[retained * PAGE + 2..retained * PAGE + 4]
                        .try_into()
                        .unwrap()
                ),
                1799
            );
            if empty_hint {
                assert_eq!(&after[19 * PAGE..19 * PAGE + 2], &[1, 1]);
                assert_eq!(word(after, 19, 4), TABLE as u32);
            }
        }
    }
}

#[test]
fn healthy_checked_child_growth_rejects_residue_requests_and_extra_patch_authority() {
    for target_left in [false, true] {
        let before = pressured(5, target_left, 19, 13, false);
        checked(&before, 452);
        let image = capture(&before);
        let requested = requests(&[0]);
        let structural = preflight(&image, "Items", &requested, &mut budget(), &|| false).unwrap();
        let mut staged =
            prepare(&image, &structural, &requested, &mut budget(), &|| false).unwrap();
        let healthy = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
        audit::prove(
            &image,
            "Items",
            &requested,
            &staged,
            &healthy,
            &mut budget(),
            &|| false,
        )
        .unwrap();

        staged.indexes.added[0].bytes[3000] = 0x58;
        let forged = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
        checked(&forged, 453);
        assert_eq!(
            audit::prove(
                &image,
                "Items",
                &requested,
                &staged,
                &forged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        staged.indexes.added[0].bytes[3000] = 0;

        for patch in 0..staged.indexes.patches.len() {
            staged.indexes.patches[patch].after[3000] ^= 1;
            let forged = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
            checked(&forged, 453);
            assert_eq!(
                audit::prove(
                    &image,
                    "Items",
                    &requested,
                    &staged,
                    &forged,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            staged.indexes.patches[patch].after[3000] ^= 1;
        }
        if target_left {
            let before_page = before[18 * PAGE..19 * PAGE].to_vec();
            let mut after_page = before_page.clone();
            after_page[3000] ^= 1;
            staged.indexes.patches.push(IndexDataPatch {
                offset: 18 * PAGE,
                before: before_page,
                after: after_page,
            });
            let forged = compose::compose(&image, &staged, &mut budget(), &|| false).unwrap();
            checked(&forged, 453);
            assert_eq!(
                audit::prove(
                    &image,
                    "Items",
                    &requested,
                    &staged,
                    &forged,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            staged.indexes.patches.pop();
        }
        let drift = requests(&[-227]);
        assert_eq!(
            audit::prove(
                &image,
                "Items",
                &drift,
                &staged,
                &healthy,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn two_offenders_and_a_still_overflowing_half_refuse_without_a_candidate() {
    let left = (0..451)
        .map(|number| i32::MIN + number * 1_000_000)
        .collect::<Vec<_>>();
    let right = (0..451)
        .map(|number| number * 1_000_000)
        .collect::<Vec<_>>();
    let before = source(5, &left, &right, 128, 95, false);
    checked(&before, 902);
    let image = capture(&before);
    let both = requests(&[i32::MIN + 500_000, 500_000]);
    let structural = preflight(&image, "Items", &both, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        prepare(&image, &structural, &both, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));

    let before = pressured(5, true, 19, 13, false);
    checked(&before, 452);
    let image = capture(&before);
    let values = (1..=452)
        .map(|number| -number * 1_000_000)
        .collect::<Vec<_>>();
    let requests = requests(&values);
    let structural = preflight(&image, "Items", &requests, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        prepare(&image, &structural, &requests, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
    ));
}
