//! Independent complete images for ordinary hint and index map transitions.
use super::purpose;
use crate::{
    SlotScalarUpdate,
    test_support::encoded::{model, split_model},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const DATA: [usize; 5] = [12, 15, 16, 17, 18];
const ROW_BYTES: usize = 69;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn install(bytes: &mut [u8], page: usize, block: &[u8]) {
    assert_eq!(block.len(), PAGE);
    bytes[page * PAGE..(page + 1) * PAGE].copy_from_slice(block);
}

fn inline(base: usize, members: &[usize]) -> Vec<u8> {
    let mut row = vec![0; ROW_BYTES];
    row[1..5].copy_from_slice(&(base as u32).to_le_bytes());
    for &number in members {
        let relative = number.checked_sub(base).unwrap();
        assert!(relative < 512);
        row[5 + relative / 8] |= 1 << (relative % 8);
    }
    row
}

fn indirect(number: usize) -> Vec<u8> {
    let mut row = vec![0; ROW_BYTES];
    row[0] = 1;
    row[1..5].copy_from_slice(&(number as u32).to_le_bytes());
    row
}

fn bitmap(members: impl IntoIterator<Item = usize>) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..4].copy_from_slice(&[5, 1, 0, 0]);
    for number in members {
        assert!(number < 32736);
        bytes[4 + number / 8] |= 1 << (number % 8);
    }
    bytes
}

struct Case {
    before: Vec<u8>,
    expected: Vec<u8>,
    eof: usize,
    lower: usize,
    highest: usize,
    index_root: usize,
    index_pages: [usize; 3],
    hint_bitmap: Option<usize>,
    index_bitmap: Option<usize>,
}

fn case(code: u8, descending: bool, eof: usize, promotion: bool) -> Case {
    // The inherited factory constructs 900 genuine Binary records and complete
    // declarations. Only additional deleted DATA records are introduced here;
    // live row counts, values, physical references and owners remain literal.
    let mut before = purpose::source(code, descending, eof, 0);
    let (lower, highest, index_root) = if promotion {
        (12, 21, 13)
    } else {
        (96, 100, 95)
    };
    let mut owned = DATA.to_vec();
    for number in [lower, highest] {
        if !owned.contains(&number) {
            let body = if number == highest { 4050 } else { 3880 };
            let mut page = support::data_page(10, &[(0x8000, vec![0x9d; body])]);
            let free = PAGE - body - 16;
            support::put16(&mut page, 2, free as u16);
            assert!(free < 270);
            install(&mut before, number, &page);
            owned.push(number);
        }
    }
    owned.sort_unstable();
    if index_root != 13 {
        let page = before[13 * PAGE..14 * PAGE].to_vec();
        install(&mut before, index_root, &page);
        // The real physical index descriptor is rebound with its map. Both
        // logical index names continue to share this same descriptor.
        support::put32(&mut before, 10 * PAGE + 181, index_root as u32);
        install(
            &mut before,
            14,
            &support::data_page(0, &[(0, inline(0, &[index_root]))]),
        );
    }
    install(&mut before, eof - 3, &bitmap(owned.iter().copied()));
    install(
        &mut before,
        11,
        &support::data_page(
            0,
            &[(0, indirect(eof - 3)), (0, inline(0, &[lower, highest]))],
        ),
    );
    // The previously referenced hint bitmap remains allocated, unchanged, and
    // unreferenced. No map page is reclaimed by this operation.
    let old_hint = before[(eof - 2) * PAGE..(eof - 1) * PAGE].to_vec();
    let hint_bitmap = promotion.then_some(eof + 1);
    let index_bitmap = promotion.then_some(eof + 3);
    let index_pages = if promotion {
        [eof + 2, eof + 4, eof + 5]
    } else {
        [eof + 1, eof + 2, eof + 3]
    };
    let final_eof = eof + if promotion { 6 } else { 4 };
    let mut expected = before.clone();
    expected.resize(final_eof * PAGE, 0);

    // Exact original record reconstruction: 6-byte fixed prefix, 255 payload
    // bytes, two offsets, physical variable count and present mask.
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
    let root_start = usize::from(u16::from_le_bytes(
        before[12 * PAGE + 14..12 * PAGE + 16].try_into().unwrap(),
    ));
    assert_eq!(PAGE - root_start, 14);
    expected[12 * PAGE + root_start..13 * PAGE].fill(0);
    support::put32(&mut expected, 12 * PAGE + root_start, (eof as u32) << 8);
    support::put16(&mut expected, 12 * PAGE + 14, root_start as u16 | 0x4000);

    let old_key = split_model::binary_key(&[0], descending);
    let new_key = split_model::binary_key(&[255; 255], descending);
    let mut entries = (0..900)
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
    let mut bounds = Vec::new();
    let mut start = 0;
    for (position, (&page, count)) in order.iter().zip(counts).enumerate() {
        let subset = &entries[start..start + count];
        let block = purpose::index_page(
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
        install(&mut expected, page, &block);
        if position < 2 {
            bounds.push(
                [
                    subset.last().unwrap().as_slice(),
                    &(page as u32).to_be_bytes(),
                ]
                .concat(),
            );
        }
        start += count;
    }
    let root = purpose::index_page(
        &bounds,
        0,
        0,
        index_pages[0],
        Some(&before[index_root * PAGE..(index_root + 1) * PAGE]),
    );
    install(&mut expected, index_root, &root);
    owned.push(eof);
    install(&mut expected, eof - 3, &bitmap(owned));
    let hint_row = if let Some(number) = hint_bitmap {
        install(&mut expected, number, &bitmap([lower, eof]));
        indirect(number)
    } else {
        let base = lower / 8 * 8;
        assert!(eof - base + 1 < 512);
        inline(base, &[lower, eof])
    };
    install(
        &mut expected,
        11,
        &support::data_page(0, &[(0, indirect(eof - 3)), (0, hint_row)]),
    );
    let index_members = [index_root]
        .into_iter()
        .chain(index_pages)
        .collect::<Vec<_>>();
    let index_row = if let Some(number) = index_bitmap {
        install(&mut expected, number, &bitmap(index_members));
        indirect(number)
    } else {
        let base = index_root / 8 * 8;
        assert!(index_pages[2] - base + 1 < 512);
        inline(base, &index_members)
    };
    install(&mut expected, 14, &support::data_page(0, &[(0, index_row)]));
    install(&mut expected, eof - 1, &bitmap(final_eof..32736));
    assert_eq!(&expected[(eof - 2) * PAGE..(eof - 1) * PAGE], old_hint);
    Case {
        before,
        expected,
        eof,
        lower,
        highest,
        index_root,
        index_pages,
        hint_bitmap,
        index_bitmap,
    }
}

fn accepted(case: Case) {
    // Validate both independently constructed images before invoking the writer.
    purpose::checked(&case.before);
    purpose::checked(&case.expected);
    let request = [SlotScalarUpdate::new(
        0,
        Value::Binary(vec![0]),
        Value::Binary(vec![255; 255]),
    )];
    let result = model::capture(&case.before)
        .preview_ace16_variable_split_updates("Items", "Score", &request, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(result.before_bytes(), case.before);
    assert_eq!(result.candidate_bytes(), case.expected);
    assert_eq!(result.changed_rows(), 1);
    let bytes = result.candidate_bytes();
    let hint_members = if let Some(bitmap) = case.hint_bitmap {
        &bytes[bitmap * PAGE + 4..(bitmap + 1) * PAGE]
    } else {
        // Both rows have equal physical widths, so the first row's start remains
        // 4027 and the second row's start remains 3958 after full rewrites.
        let base = case.lower / 8 * 8;
        let row = &bytes[11 * PAGE + 3958..11 * PAGE + 4027];
        assert_eq!(&row[..5], &inline(base, &[])[..5]);
        assert_eq!(
            row[5 + (case.lower - base) / 8] & (1 << ((case.lower - base) % 8)),
            1 << ((case.lower - base) % 8)
        );
        assert_eq!(
            row[5 + (case.highest - base) / 8] & (1 << ((case.highest - base) % 8)),
            0
        );
        &[]
    };
    for page in case
        .index_pages
        .into_iter()
        .chain(case.index_bitmap)
        .chain(case.hint_bitmap)
    {
        assert_eq!(
            bytes[(case.eof - 3) * PAGE + 4 + page / 8] & (1 << (page % 8)),
            0
        );
        if case.hint_bitmap.is_some() {
            assert_eq!(hint_members[page / 8] & (1 << (page % 8)), 0);
        }
    }
    assert_eq!(
        bytes[(case.eof - 1) * PAGE + 4 + case.eof / 8] & (1 << (case.eof % 8)),
        0
    );
    assert!(case.index_root < case.eof);
}

#[test]
fn failed_highest_hint_preserves_lower_and_rebases_both_complete_map_rows() {
    for code in [5, 6] {
        for descending in [false, true] {
            for eof in [537, 550] {
                accepted(case(code, descending, eof, false));
            }
        }
    }
}

#[test]
fn candidate_bitmap_precedes_index_promotion_without_entering_membership() {
    for code in [5, 6] {
        for descending in [false, true] {
            accepted(case(code, descending, 600, true));
        }
    }
}

#[test]
fn complete_hint_replay_rejects_missing_lower_restored_highest_and_metadata_owner_bits() {
    use super::super::{finish, preflight, prepare};
    use crate::{EditError, table::mutation::variable_update::authority::Operation};

    for mutation in 0..3 {
        let case = case(
            6,
            false,
            if mutation == 2 { 600 } else { 537 },
            mutation == 2,
        );
        purpose::checked(&case.before);
        purpose::checked(&case.expected);
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

        let (offset, mask, set) = match mutation {
            0 => (11 * PAGE + 3958 + 5, 1, false),
            1 => (11 * PAGE + 3958 + 5, 1 << 4, true),
            _ => ((case.eof - 3) * PAGE + 4 + 601 / 8, 1 << (601 % 8), true),
        };
        let byte = staged
            .indexes
            .append
            .map_edits
            .iter_mut()
            .find(|byte| byte.offset == offset)
            .unwrap();
        if set {
            assert_eq!(byte.after & mask, 0);
            byte.after |= mask;
        } else {
            assert_eq!(byte.after & mask, mask);
            byte.after &= !mask;
        }
        assert_eq!(
            finish::finish(
                model::capture(&case.before),
                &operation,
                &staged,
                &mut budget(),
                &|| false
            )
            .err(),
            Some(EditError::Preservation),
            "mutation {mutation}",
        );
    }
}
