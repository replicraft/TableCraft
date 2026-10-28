//! DATA/map stage goldens; table/index counters belong to the parent composer.
use super::*;
use crate::{
    IndexCheckStatus, check_table, check_table_constraints,
    test_support::{
        encoded::{model, split_model},
        map_growth,
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::LimitError;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const DATA: [usize; 5] = [12, 15, 16, 17, 18];

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn inline(members: &[usize]) -> Vec<u8> {
    let mut row = vec![0; 69];
    for &number in members {
        assert!(number < 512);
        row[5 + number / 8] |= 1 << (number % 8);
    }
    row
}

fn indirect(number: usize) -> Vec<u8> {
    let mut row = vec![0; 69];
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

fn install(bytes: &mut [u8], number: usize, page: &[u8]) {
    assert_eq!(page.len(), PAGE);
    bytes[number * PAGE..(number + 1) * PAGE].copy_from_slice(page);
}

fn source(code: u8, eof: usize, filled: bool, extras: usize, empty_tombstone: bool) -> Vec<u8> {
    let seed = split_model::source(split_model::Family::Binary, false, true);
    let mut bytes = seed.bytes;
    bytes.resize(eof * PAGE, 0);
    bytes[20] = code;
    for (position, &number) in DATA.iter().enumerate() {
        let mut rows = seed.records[position * 180..(position + 1) * 180]
            .iter()
            .cloned()
            .map(|record| (0, record))
            .collect::<Vec<_>>();
        if filled {
            rows.push((0x8000, vec![0x9d; 1000]));
        }
        if number == 18 {
            rows.extend((0..extras).map(|_| (0x8000, vec![0xa7])));
            if empty_tombstone {
                rows.push((0xc000, Vec::new()));
            }
        }
        let total: usize = rows.iter().map(|(_, row)| row.len()).sum();
        let free = PAGE - total - 14 - rows.len() * 2;
        let mut page = support::data_page(10, &rows);
        support::put16(&mut page, 2, free as u16);
        install(&mut bytes, number, &page);
    }
    support::put32(&mut bytes, 10 * PAGE + 59, (11 << 8) | 1);
    if eof == 24 {
        install(
            &mut bytes,
            11,
            &support::data_page(0, &[(0, inline(&DATA)), (0, inline(&[18]))]),
        );
        install(
            &mut bytes,
            1,
            &support::data_page(1, &[(0, inline(&(eof..512).collect::<Vec<_>>()))]),
        );
    } else {
        install(
            &mut bytes,
            11,
            &support::data_page(0, &[(0, indirect(eof - 3)), (0, inline(&[12, 18]))]),
        );
        install(&mut bytes, eof - 3, &bitmap(DATA));
        install(&mut bytes, eof - 2, &bitmap([18]));
        install(
            &mut bytes,
            1,
            &support::data_page(1, &[(0, indirect(eof - 1))]),
        );
        install(&mut bytes, eof - 1, &bitmap(eof..32736));
    }
    for number in [2, 7, 10] {
        let end = map_growth::model::declaration_end(&bytes, number);
        support::put16(&mut bytes, end, 0xffff);
    }
    bytes
}

fn checked(bytes: &[u8]) -> crate::TableCheck {
    let check = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(check.rows(), 900);
    assert!(
        check
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    assert_eq!(
        check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        900
    );
    check
}

fn metadata(image: &NativeImage, check: &crate::TableCheck) -> Metadata {
    Metadata::capture(
        image,
        check.schema(),
        check.indexes(),
        1,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn record(id: i32) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend(id.to_le_bytes());
    bytes.extend([0xff; 255]);
    bytes.extend([5, 1, 6, 0, 1, 0, 3]);
    assert_eq!(bytes.len(), 268);
    bytes
}

fn literal_append(page: &mut [u8], record: &[u8]) -> (usize, usize) {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    let end = if count == 0 {
        PAGE
    } else {
        usize::from(
            u16::from_le_bytes(
                page[14 + 2 * (count - 1)..16 + 2 * (count - 1)]
                    .try_into()
                    .unwrap(),
            ) & 0x1fff,
        )
    };
    let start = end - record.len();
    assert!(start >= 16 + 2 * count);
    page[start..end].copy_from_slice(record);
    support::put16(page, 14 + 2 * count, start as u16);
    support::put16(page, 12, (count + 1) as u16);
    support::put16(page, 2, (start - 16 - 2 * count) as u16);
    (count, start)
}

fn composed(before: &[u8], plan: &Plan) -> Vec<u8> {
    let mut after = before.to_vec();
    after.resize(plan.append.length, 0);
    for page in &plan.pages {
        let start = page.number as usize * PAGE;
        if page.before.is_empty() {
            plan.append
                .require_data(page.number, &mut budget(), &|| false)
                .unwrap();
        } else {
            assert_eq!(&before[start..start + PAGE], page.before);
        }
        after[start..start + PAGE].copy_from_slice(&page.after);
    }
    for (number, reference, bytes) in plan.maps.metadata_pages() {
        plan.append
            .require_candidate_bitmap(number, 10, reference, &mut budget(), &|| false)
            .unwrap();
        after[number as usize * PAGE..(number as usize + 1) * PAGE].copy_from_slice(bytes);
    }
    for edit in &plan.append.map_edits {
        assert_eq!(before[edit.offset], edit.before);
        after[edit.offset] = edit.after;
    }
    after
}

#[test]
fn visible_existing_page_append_preserves_every_original_record_and_map_byte() {
    for code in [5, 6] {
        let before = source(code, 24, false, 0, false);
        let check = checked(&before);
        let image = model::capture(&before);
        let records = [record(10001)];
        let mut expected = before.clone();
        let (slot, start) = literal_append(&mut expected[18 * PAGE..19 * PAGE], &records[0]);
        assert_eq!((slot, start), (180, 1308));
        let plan = prepare(
            &image,
            check.schema(),
            metadata(&image, &check),
            &records,
            State::new(&image, check.schema().definition_page()),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(composed(&before, &plan), expected);
        assert_eq!(plan.rows[0].reference, (18 << 8) | 180);
        assert_eq!(plan.rows[0].range, 18 * PAGE + 1308..18 * PAGE + 1576);
        assert_eq!(plan.append.length, before.len());
        assert!(plan.append.map_edits.is_empty());
        assert!(plan.append.indexes().next().is_none());
    }
}

#[test]
fn staged_page_reuse_then_one_highest_miss_allocates_one_visible_eof_page() {
    let before = source(6, 24, false, 0, false);
    let check = checked(&before);
    let image = model::capture(&before);
    let records = (0..5).map(|row| record(10001 + row)).collect::<Vec<_>>();
    let mut expected = before.clone();
    expected.resize(25 * PAGE, 0);
    for row in &records[..4] {
        literal_append(&mut expected[18 * PAGE..19 * PAGE], row);
    }
    let mut page = support::data_page(10, &[]);
    support::put16(&mut page, 2, 4082);
    literal_append(&mut page, &records[4]);
    install(&mut expected, 24, &page);
    install(
        &mut expected,
        11,
        &support::data_page(
            0,
            &[(0, inline(&[12, 15, 16, 17, 18, 24])), (0, inline(&[24]))],
        ),
    );
    install(
        &mut expected,
        1,
        &support::data_page(1, &[(0, inline(&(25..512).collect::<Vec<_>>()))]),
    );
    let plan = prepare(
        &image,
        check.schema(),
        metadata(&image, &check),
        &records,
        State::new(&image, check.schema().definition_page()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(composed(&before, &plan), expected);
    assert_eq!(
        plan.rows
            .iter()
            .map(|row| row.reference)
            .collect::<Vec<_>>(),
        [
            (18 << 8) | 180,
            (18 << 8) | 181,
            (18 << 8) | 182,
            (18 << 8) | 183,
            24 << 8
        ]
    );
    assert_eq!(
        plan.pages
            .iter()
            .filter(|page| page.before.is_empty())
            .count(),
        1
    );
    assert!(plan.append.indexes().next().is_none());
}

#[test]
fn physical_slot_254_can_become_255_but_next_insert_uses_new_page_without_reuse() {
    let before = source(6, 24, false, 74, false);
    let check = checked(&before);
    let image = model::capture(&before);
    let records = [record(10001), record(10002)];
    let mut expected = before.clone();
    expected.resize(25 * PAGE, 0);
    let (slot, _) = literal_append(&mut expected[18 * PAGE..19 * PAGE], &records[0]);
    assert_eq!(slot, 254);
    let mut page = support::data_page(10, &[]);
    support::put16(&mut page, 2, 4082);
    literal_append(&mut page, &records[1]);
    install(&mut expected, 24, &page);
    install(
        &mut expected,
        11,
        &support::data_page(
            0,
            &[(0, inline(&[12, 15, 16, 17, 18, 24])), (0, inline(&[24]))],
        ),
    );
    install(
        &mut expected,
        1,
        &support::data_page(1, &[(0, inline(&(25..512).collect::<Vec<_>>()))]),
    );
    let plan = prepare(
        &image,
        check.schema(),
        metadata(&image, &check),
        &records,
        State::new(&image, check.schema().definition_page()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(composed(&before, &plan), expected);
    assert_eq!(plan.rows[0].reference, (18 << 8) | 254);
    assert_eq!(plan.rows[1].reference, 24 << 8);
}

#[test]
fn deleted_zero_length_slot_is_preserved_but_live_empty_or_bad_mask_geometry_refuses() {
    let before = source(6, 24, false, 0, true);
    let check = checked(&before);
    let image = model::capture(&before);
    let records = [record(10001)];
    let mut expected = before.clone();
    literal_append(&mut expected[18 * PAGE..19 * PAGE], &records[0]);
    let plan = prepare(
        &image,
        check.schema(),
        metadata(&image, &check),
        &records,
        State::new(&image, check.schema().definition_page()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(composed(&before, &plan), expected);
    assert_eq!(plan.rows[0].reference, (18 << 8) | 181);
    let mut malformed = before[18 * PAGE..19 * PAGE].to_vec();
    let word = u16::from_le_bytes(malformed[374..376].try_into().unwrap());
    support::put16(&mut malformed, 374, word & 0x1fff);
    assert_eq!(
        geometry(&malformed, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Malformed))
    );
    support::put16(&mut malformed, 374, word | 0x2000);
    assert_eq!(
        geometry(&malformed, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Unsupported))
    );
    support::put16(&mut malformed, 374, word);
    malformed[2] ^= 1;
    assert_eq!(
        geometry(&malformed, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Malformed))
    );
}

#[test]
fn candidate_bitmap_is_separate_from_visible_data_and_all_membership() {
    let before = source(6, 600, true, 0, false);
    let check = checked(&before);
    let image = model::capture(&before);
    let records = [record(10001)];
    let mut expected = before.clone();
    expected.resize(602 * PAGE, 0);
    let mut page = support::data_page(10, &[]);
    support::put16(&mut page, 2, 4082);
    literal_append(&mut page, &records[0]);
    install(&mut expected, 600, &page);
    install(&mut expected, 601, &bitmap([12, 600]));
    install(
        &mut expected,
        11,
        &support::data_page(0, &[(0, indirect(597)), (0, indirect(601))]),
    );
    install(&mut expected, 597, &bitmap(DATA.into_iter().chain([600])));
    install(&mut expected, 599, &bitmap(602..32736));
    let plan = prepare(
        &image,
        check.schema(),
        metadata(&image, &check),
        &records,
        State::new(&image, check.schema().definition_page()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(composed(&before, &plan), expected);
    assert_eq!(plan.rows[0].reference, 600 << 8);
    assert_eq!(
        plan.maps
            .metadata_pages()
            .map(|(number, _, _)| number)
            .collect::<Vec<_>>(),
        [601]
    );
    assert!(
        plan.append
            .require_data(601, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(
        &expected[598 * PAGE..599 * PAGE],
        &before[598 * PAGE..599 * PAGE]
    );
    assert_eq!(expected[597 * PAGE + 4 + 601 / 8] & 2, 0);
    assert_eq!(expected[601 * PAGE + 4 + 601 / 8] & 2, 0);
    assert!(plan.append.indexes().next().is_none());
}

#[test]
fn cumulative_pools_cancellation_and_retry_never_modify_the_original_image() {
    let before = source(6, 24, false, 0, false);
    let check = checked(&before);
    let image = model::capture(&before);
    let records = [record(10001)];
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut allowance = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 100_000_000),
            Resource::Rows => ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000),
            Resource::Steps => ResourceBudget::new(256 * 1024 * 1024, 100_000, 0),
        };
        assert!(matches!(
            prepare(
                &image,
                check.schema(),
                metadata(&image, &check),
                &records,
                State::new(&image, check.schema().definition_page()),
                &mut allowance,
                &|| false,
            ),
            Err(EditError::Inspect(InspectError::Limit(LimitError {
                resource: actual,
                ..
            }))) if actual == resource
        ));
    }
    let calls = Cell::new(0);
    let cancelled = || {
        let prior = calls.get();
        calls.set(prior + 1);
        prior >= 20
    };
    assert!(matches!(
        prepare(
            &image,
            check.schema(),
            metadata(&image, &check),
            &records,
            State::new(&image, check.schema().definition_page()),
            &mut budget(),
            &cancelled
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let retry = prepare(
        &image,
        check.schema(),
        metadata(&image, &check),
        &records,
        State::new(&image, check.schema().definition_page()),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(retry.rows.len(), 1);
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn empty_request_keeps_all_pages_maps_and_cursor_unchanged() {
    for code in [5, 6] {
        for eof in [24, 600] {
            let before = source(code, eof, false, 0, false);
            let check = checked(&before);
            let image = model::capture(&before);
            let plan = prepare(
                &image,
                check.schema(),
                metadata(&image, &check),
                &[],
                State::new(&image, check.schema().definition_page()),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(plan.rows.is_empty());
            assert!(plan.pages.is_empty());
            assert!(plan.append.map_edits.is_empty());
            assert_eq!(plan.append.length, before.len());
            assert_eq!(plan.maps.metadata_pages().count(), 0);
            assert_eq!(composed(&before, &plan), before);
            assert_eq!(image.as_bytes(), before);
        }
    }
}
