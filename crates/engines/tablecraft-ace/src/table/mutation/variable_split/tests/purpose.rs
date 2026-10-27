//! Literal DATA/index/bitmap purposes must compose without crossing membership.
use super::super::{finish, preflight, prepare};
use crate::table::mutation::variable_update::authority::Operation;
use crate::{
    EditError, IndexCheckStatus, InspectError, SlotScalarUpdate, check_table,
    check_table_constraints, read_table,
    test_support::{
        encoded::{model, split_model},
        map_growth, support,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const ROOT: usize = 13;
const DATA: [usize; 5] = [12, 15, 16, 17, 18];
const ROWS: usize = 900;
const ENTRY: usize = 480;
const CAPACITY: usize = 3616;
const MAP_BYTES: usize = 69;

struct Case {
    before: Vec<u8>,
    expected: Vec<u8>,
    eof: usize,
    owned_bitmap: Option<usize>,
    hint_bitmap: Option<usize>,
    global_bitmap: Option<usize>,
    index_pages: [usize; 3],
    index_bitmap: Option<usize>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn number(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn install(bytes: &mut [u8], page: usize, block: &[u8]) {
    assert_eq!(block.len(), PAGE);
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(block);
}

fn inline(base: usize, members: &[usize]) -> Vec<u8> {
    let mut bytes = vec![0; MAP_BYTES];
    bytes[1..5].copy_from_slice(&(base as u32).to_le_bytes());
    for &page in members {
        let relative = page.checked_sub(base).unwrap();
        assert!(relative < 512);
        bytes[5 + relative / 8] |= 1 << (relative % 8);
    }
    bytes
}

fn indirect(bitmap: usize) -> Vec<u8> {
    let mut bytes = vec![0; MAP_BYTES];
    bytes[0] = 1;
    bytes[1..5].copy_from_slice(&(bitmap as u32).to_le_bytes());
    bytes
}

fn bitmap(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..4].copy_from_slice(&[5, 1, 0, 0]);
    for page in members {
        assert!(page < 32736);
        bytes[4 + page / 8] |= 1 << (page % 8);
    }
    bytes
}

fn common(entries: &[Vec<u8>], node: bool) -> usize {
    let mut prefix = entries[0].len() - if node { 4 } else { 0 };
    for entry in &entries[1..] {
        prefix = prefix.min(entry.len() - if node { 4 } else { 0 });
        prefix = entries[0][..prefix]
            .iter()
            .zip(&entry[..prefix])
            .take_while(|(left, right)| left == right)
            .count();
    }
    prefix
}

fn index_page(
    entries: &[Vec<u8>],
    previous: usize,
    next: usize,
    tail: usize,
    original: Option<&[u8]>,
) -> Vec<u8> {
    let node = tail != 0;
    let mut bytes = original.map_or_else(|| vec![0; PAGE], <[u8]>::to_vec);
    bytes[..2].copy_from_slice(&[if node { 3 } else { 4 }, 1]);
    support::put32(&mut bytes, 4, 10);
    support::put32(&mut bytes, 12, previous as u32);
    support::put32(&mut bytes, 16, next as u32);
    support::put32(&mut bytes, 20, tail as u32);
    bytes[26] = u8::from(node);
    bytes[27..ENTRY].fill(0);
    let total: usize = entries.iter().map(Vec::len).sum();
    let prefix = if total <= CAPACITY {
        0
    } else {
        common(entries, node)
    };
    let active = total - prefix * (entries.len() - 1);
    assert!(active <= CAPACITY);
    support::put16(&mut bytes, 2, (CAPACITY - active) as u16);
    support::put16(&mut bytes, 24, prefix as u16);
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        let suffix = &entry[if position == 0 { 0 } else { prefix }..];
        bytes[ENTRY + end..ENTRY + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        bytes[27 + end / 8] |= 1 << (end % 8);
    }
    assert_eq!(end, active);
    if let Some(original) = original {
        assert_eq!(&bytes[ENTRY + active..], &original[ENTRY + active..]);
    } else {
        assert!(bytes[ENTRY + active..].iter().all(|byte| *byte == 0));
    }
    bytes
}

fn source(code: u8, descending: bool, eof: usize, index_base: usize) -> Vec<u8> {
    let seed = split_model::source(split_model::Family::Binary, descending, true);
    let mut bytes = seed.bytes;
    bytes.resize(eof * PAGE, 0);
    bytes[20] = code;
    for (position, &page) in DATA.iter().enumerate() {
        let mut rows = seed.records[position * 180..(position + 1) * 180]
            .iter()
            .cloned()
            .map(|record| (0, record))
            .collect::<Vec<_>>();
        rows.push((0x8000, vec![0x9d; 1000]));
        let mut block = support::data_page(10, &rows);
        let end = number(&block, 14 + 180 * 2) & 0x1fff;
        let free = end - 14 - 181 * 2;
        assert_eq!(free, 200);
        support::put16(&mut block, 2, free as u16);
        install(&mut bytes, page, &block);
    }
    support::put32(&mut bytes, 10 * PAGE + 59, (11 << 8) | 1);
    if eof == 24 {
        let owned = inline(0, &DATA);
        let hint = inline(0, &[18]);
        install(
            &mut bytes,
            11,
            &support::data_page(0, &[(0, owned), (0, hint)]),
        );
        let global = inline(0, &(eof..512).collect::<Vec<_>>());
        install(&mut bytes, 1, &support::data_page(1, &[(0, global)]));
    } else {
        let owned_bitmap = eof - 3;
        let hint_bitmap = eof - 2;
        let global_bitmap = eof - 1;
        install(
            &mut bytes,
            11,
            &support::data_page(
                0,
                &[(0, indirect(owned_bitmap)), (0, indirect(hint_bitmap))],
            ),
        );
        install(&mut bytes, owned_bitmap, &bitmap(DATA));
        install(&mut bytes, hint_bitmap, &bitmap([18]));
        install(
            &mut bytes,
            1,
            &support::data_page(1, &[(0, indirect(global_bitmap))]),
        );
        install(&mut bytes, global_bitmap, &bitmap(eof..32736));
    }
    install(
        &mut bytes,
        14,
        &support::data_page(0, &[(0, inline(index_base, &[ROOT]))]),
    );
    for definition in [2, 7, 10] {
        let end = map_growth::model::declaration_end(&bytes, definition);
        support::put16(&mut bytes, end, 0xffff);
    }
    bytes
}

fn case(code: u8, descending: bool, eof: usize, index_base: usize) -> Case {
    let before = source(code, descending, eof, index_base);
    let promotion = eof != 24;
    let late = eof == 523;
    let index_pages = if late {
        [524, 525, 527]
    } else if promotion {
        [601, 603, 604]
    } else {
        [25, 26, 27]
    };
    let index_bitmap = if late {
        Some(526)
    } else if promotion {
        Some(602)
    } else {
        None
    };
    let mut expected = before.clone();
    expected.resize((eof + if promotion { 5 } else { 4 }) * PAGE, 0);
    let mut record = vec![2, 0, 1, 0, 0, 0];
    record.extend([255; 255]);
    record.extend(261_u16.to_le_bytes());
    record.extend(6_u16.to_le_bytes());
    record.extend(1_u16.to_le_bytes());
    record.push(3);
    assert_eq!(record.len(), 268);
    let mut data = vec![0; PAGE];
    data[..2].copy_from_slice(&[1, 1]);
    support::put16(&mut data, 2, 3812);
    support::put32(&mut data, 4, 10);
    support::put16(&mut data, 12, 1);
    support::put16(&mut data, 14, 0x8000 | 3828);
    data[3828..].copy_from_slice(&record);
    install(&mut expected, eof, &data);
    let root_start = number(&before, 12 * PAGE + 14);
    assert_eq!(PAGE - root_start, 14);
    expected[12 * PAGE + root_start..13 * PAGE].fill(0);
    support::put32(&mut expected, 12 * PAGE + root_start, (eof as u32) << 8);
    support::put16(&mut expected, 12 * PAGE + 14, root_start as u16 | 0x4000);
    let new_key = split_model::binary_key(&[255; 255], descending);
    let old_key = split_model::binary_key(&[0], descending);
    let mut entries = (0..ROWS)
        .map(|row| {
            let reference = ((DATA[row / 180] as u32) << 8) | (row % 180) as u32;
            [
                if row == 0 {
                    new_key.as_slice()
                } else {
                    old_key.as_slice()
                },
                &reference.to_be_bytes(),
            ]
            .concat()
        })
        .collect::<Vec<_>>();
    entries.sort();
    let order = if descending {
        [index_pages[2], index_pages[1], index_pages[0]]
    } else {
        [index_pages[1], index_pages[2], index_pages[0]]
    };
    let counts = if descending {
        [225, 225, 450]
    } else {
        [450, 225, 225]
    };
    let mut at = 0;
    let mut bounds = Vec::new();
    for (position, (&page, &count)) in order.iter().zip(&counts).enumerate() {
        let subset = &entries[at..at + count];
        let leaf = index_page(
            subset,
            if position == 0 {
                0
            } else {
                order[position - 1]
            },
            order.get(position + 1).copied().unwrap_or(0),
            0,
            None,
        );
        install(&mut expected, page, &leaf);
        if position < 2 {
            bounds.push(
                [
                    subset.last().unwrap().as_slice(),
                    &(page as u32).to_be_bytes(),
                ]
                .concat(),
            );
        }
        at += count;
    }
    let root = index_page(
        &bounds,
        0,
        0,
        index_pages[0],
        Some(&before[ROOT * PAGE..(ROOT + 1) * PAGE]),
    );
    install(&mut expected, ROOT, &root);
    if promotion {
        install(
            &mut expected,
            eof - 3,
            &bitmap(DATA.into_iter().chain([eof])),
        );
        install(&mut expected, eof - 2, &bitmap([eof]));
        let final_pages = expected.len() / PAGE;
        install(&mut expected, eof - 1, &bitmap(final_pages..32736));
        install(
            &mut expected,
            14,
            &support::data_page(0, &[(0, indirect(index_bitmap.unwrap()))]),
        );
        install(
            &mut expected,
            index_bitmap.unwrap(),
            &bitmap([ROOT].into_iter().chain(index_pages)),
        );
    } else {
        install(
            &mut expected,
            11,
            &support::data_page(
                0,
                &[
                    (
                        0,
                        inline(0, &DATA.into_iter().chain([eof]).collect::<Vec<_>>()),
                    ),
                    (0, inline(0, &[eof])),
                ],
            ),
        );
        install(
            &mut expected,
            1,
            &support::data_page(1, &[(0, inline(0, &(28..512).collect::<Vec<_>>()))]),
        );
        install(
            &mut expected,
            14,
            &support::data_page(0, &[(0, inline(0, &[ROOT, 25, 26, 27]))]),
        );
    }
    Case {
        before,
        expected,
        eof,
        owned_bitmap: promotion.then_some(eof - 3),
        hint_bitmap: promotion.then_some(eof - 2),
        global_bitmap: promotion.then_some(eof - 1),
        index_pages,
        index_bitmap,
    }
}

fn checked(bytes: &[u8]) {
    let rows = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), ROWS);
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
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(report.indexes().len(), 2);
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), ROWS);
    assert!(constraints.relationships().is_empty());
}

fn accepted(case: &Case) {
    checked(&case.before);
    checked(&case.expected);
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![0]),
        Value::Binary(vec![255; 255]),
    )];
    let result = model::capture(&case.before)
        .preview_ace16_variable_split_updates("Items", "Score", &requests, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(result.before_bytes(), case.before);
    assert_eq!(result.candidate_bytes(), case.expected);
    assert_eq!(result.changed_rows(), 1);
    checked(result.candidate_bytes());
    for page in case.index_pages.into_iter().chain(case.index_bitmap) {
        let membership = if let Some(bitmap) = case.owned_bitmap {
            case.expected[bitmap * PAGE + 4 + page / 8] & (1 << (page % 8))
        } else {
            case.expected[11 * PAGE + 4027 + 5 + page / 8] & (1 << (page % 8))
        };
        assert_eq!(
            membership, 0,
            "Index/bitmap page entered ordinary ownership"
        );
    }
    if let Some(bitmap) = case.hint_bitmap {
        for page in case.index_pages.into_iter().chain(case.index_bitmap) {
            assert_eq!(
                case.expected[bitmap * PAGE + 4 + page / 8] & (1 << (page % 8)),
                0
            );
        }
    }
    if let Some(bitmap) = case.global_bitmap {
        for page in case.eof..case.expected.len() / PAGE {
            assert_eq!(
                case.expected[bitmap * PAGE + 4 + page / 8] & (1 << (page % 8)),
                0
            );
        }
    }
}

#[test]
fn data_and_index_share_global_byte_without_crossing_ordinary_membership() {
    for code in [5, 6] {
        for descending in [false, true] {
            accepted(&case(code, descending, 24, 0));
        }
    }
}

#[test]
fn data_precedes_index_bitmap_promotion_with_exact_separate_purposes() {
    for code in [5, 6] {
        for descending in [false, true] {
            accepted(&case(code, descending, 600, 0));
        }
    }
}

#[test]
fn late_promotion_retires_only_prior_index_bits_after_data_phase() {
    for code in [5, 6] {
        accepted(&case(code, false, 523, 13));
    }
}

#[test]
fn staged_purpose_and_correlated_membership_cannot_override_original_requests() {
    for mutation in 0..6 {
        let case = if mutation == 1 {
            case(6, false, 600, 0)
        } else {
            case(6, false, 24, 0)
        };
        checked(&case.before);
        checked(&case.expected);
        let requests = [SlotScalarUpdate::new(
            0,
            Value::Binary(vec![0]),
            Value::Binary(vec![255; 255]),
        )];
        let operation = Operation::new("Items", "Score", &requests);
        let image = model::capture(&case.before);
        let (structural, column) = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
        let mut staged = prepare(
            &image,
            &operation,
            &structural,
            column,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let healthy = finish::finish(image, &operation, &staged, &mut budget(), &|| false).unwrap();
        assert_eq!(healthy.candidate_bytes(), case.expected);

        match mutation {
            0 => {
                let page = staged
                    .indexes
                    .added
                    .iter_mut()
                    .find(|page| page.number as usize == case.index_pages[0])
                    .unwrap();
                page.bytes[..2].copy_from_slice(&[1, 1]);
            }
            1 => {
                let page = staged
                    .indexes
                    .added
                    .iter_mut()
                    .find(|page| Some(page.number as usize) == case.index_bitmap)
                    .unwrap();
                page.bytes[..2].copy_from_slice(&[1, 1]);
                support::put32(&mut page.bytes, 4, 10);
            }
            2 => {
                let page = staged
                    .data
                    .pages
                    .iter_mut()
                    .find(|page| page.number as usize == case.eof)
                    .unwrap();
                page.after[..2].copy_from_slice(&[4, 1]);
            }
            3 => {
                let offset = 11 * PAGE + 4027 + 5 + 24 / 8;
                let byte = staged
                    .indexes
                    .append
                    .map_edits
                    .iter_mut()
                    .find(|byte| byte.offset == offset)
                    .unwrap();
                byte.after |= 1 << (25 % 8);
            }
            4 => {
                let offset = 14 * PAGE + 4027 + 5 + 24 / 8;
                let byte = staged
                    .indexes
                    .append
                    .map_edits
                    .iter_mut()
                    .find(|byte| byte.offset == offset)
                    .unwrap();
                byte.after |= 1 << (24 % 8);
            }
            _ => staged.data.length += PAGE,
        }

        let error = finish::finish(
            model::capture(&case.before),
            &operation,
            &staged,
            &mut budget(),
            &|| false,
        )
        .err();
        let expected = match mutation {
            0 | 1 => EditError::Inspect(InspectError::Malformed),
            _ => EditError::Preservation,
        };
        assert_eq!(error, Some(expected), "mutation {mutation}");
    }
}

#[test]
fn index_continuation_certificate_is_bound_to_original_and_candidate_identities() {
    let case = case(6, false, 24, 0);
    checked(&case.before);
    checked(&case.expected);
    let requests = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![0]),
        Value::Binary(vec![255; 255]),
    )];
    let operation = Operation::new("Items", "Score", &requests);
    let image = model::capture(&case.before);
    let (structural, column) = preflight(&image, &operation, &mut budget(), &|| false).unwrap();
    let staged = prepare(
        &image,
        &operation,
        &structural,
        column,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let data = staged.data.view(&staged.indexes.append);
    let after = crate::table::mutation::index_append::compose_data(
        &image,
        data,
        &staged.indexes,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(after, case.expected);
    let certified = crate::table::mutation::variable_update::map_audit::certify(
        &image,
        &operation,
        data,
        &after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    staged
        .indexes
        .audit_slot_after_data(&image, &certified, &after, &mut budget(), &|| false)
        .unwrap();

    let different_image = model::capture(&case.before);
    assert_eq!(different_image.as_bytes(), image.as_bytes());
    assert_eq!(
        staged.indexes.audit_slot_after_data(
            &different_image,
            &certified,
            &after,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation),
    );

    let different_after = after.clone();
    assert_eq!(different_after, after);
    assert_eq!(
        staged.indexes.audit_slot_after_data(
            &image,
            &certified,
            &different_after,
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Preservation),
    );
}
