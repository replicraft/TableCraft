//! Source-owned maps grow at exact capacity boundaries without changing refs.
use super::*;
use crate::{
    NativeImage, check_table,
    table::mutation::insert::{
        long_values,
        record::{Profile, input},
        tests::long_value_inserts::{Family, external},
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn row_start(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * 4096 + 14 + 2 * slot;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    page * 4096 + usize::from(word & 8191)
}

fn source(eof: u32) -> Vec<u8> {
    let mut bytes = external::original(6, Family::Ole).before;
    bytes.resize(eof as usize * 4096, 0);
    let lval = 16 * 4096;
    bytes[lval + 2..lval + 4].copy_from_slice(&4_u16.to_le_bytes());
    bytes[lval + 12..lval + 14].copy_from_slice(&1_u16.to_le_bytes());
    bytes[lval + 14..lval + 16].copy_from_slice(&20_u16.to_le_bytes());
    bytes[lval + 20..lval + 4096].fill(0x5a);

    // Global page1,row0 points to an existing bitmap page17. Every byte below
    // EOF is allocated, including that bitmap; future pages are free.
    let global = row_start(&bytes, 1, 0);
    bytes[global..global + 69].fill(0);
    bytes[global] = 1;
    bytes[global + 1..global + 5].copy_from_slice(&17_u32.to_le_bytes());
    bytes[17 * 4096..17 * 4096 + 4].copy_from_slice(&[5, 1, 0, 0]);
    for page in eof..32736 {
        bytes[17 * 4096 + 4 + page as usize / 8] |= 1 << (page % 8);
    }
    bytes
}

fn construct(before: &[u8]) -> (NativeImage, long_values::Plan, State) {
    let image = NativeImage::capture(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let profile = Profile::bind(checked.schema(), &mut budget(), &|| false).unwrap();
    let mut inputs = [input::prepare(
        checked.schema(),
        profile.variables(),
        &[
            Value::Int32(10),
            Value::Int32(100),
            Value::Binary(vec![0x92; 4076]),
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap()];
    let (plan, append) = long_values::prepare(
        &image,
        &checked,
        profile.variables(),
        &mut inputs,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (image, plan, append)
}

fn overlay(before: &[u8], append: &State) -> Vec<u8> {
    let mut after = before.to_vec();
    for edit in &append.map_edits {
        assert_eq!(before[edit.offset], edit.before);
        after[edit.offset] = edit.after;
    }
    after
}

#[test]
fn distance511_rebases_but_distance512_promotes_and_never_owns_its_bitmap() {
    for (eof, promoted) in [(527_u32, false), (528, true)] {
        let before = source(eof);
        let (image, plan, append) = construct(&before);
        assert_eq!(image.as_bytes(), before);
        assert_eq!(
            plan.pages
                .iter()
                .map(|page| page.number)
                .collect::<Vec<_>>(),
            [16, eof]
        );
        assert_eq!(plan.pages[0].before, plan.pages[0].after);
        let after = overlay(&before, &append);
        let owned = row_start(&before, 11, 2);
        let candidate = row_start(&before, 11, 3);
        let mut expected_candidate = [0; 69];
        let candidate_base = if promoted { 528_u32 } else { 520_u32 };
        expected_candidate[1..5].copy_from_slice(&candidate_base.to_le_bytes());
        expected_candidate[5 + (eof - candidate_base) as usize / 8] =
            1 << ((eof - candidate_base) % 8);
        assert_eq!(&after[candidate..candidate + 69], &expected_candidate);
        assert_eq!(after[candidate + 7], 0); // Old hint16 was absorbed by the rewrite.
        if promoted {
            assert_eq!(plan.metadata.len(), 1);
            let bitmap = &plan.metadata[0];
            assert_eq!(
                (bitmap.number, bitmap.column, bitmap.owned, bitmap.block),
                (529, 2, (11 << 8) | 2, 0)
            );
            let mut expected_owned = [0; 69];
            expected_owned[0] = 1;
            expected_owned[1..5].copy_from_slice(&529_u32.to_le_bytes());
            assert_eq!(&after[owned..owned + 69], &expected_owned);
            let mut expected_bitmap = vec![0; 4096];
            expected_bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
            expected_bitmap[6] = 1;
            expected_bitmap[70] = 1;
            assert_eq!(bitmap.bytes, expected_bitmap);
            assert_eq!(bitmap.bytes[70] & 2, 0); // Metadata529 is not an LVAL member.
            append
                .require_long_value_owned_bitmap(529, 2, (11 << 8) | 2, 0, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(append.length, before.len() + 2 * 4096);
        } else {
            assert!(plan.metadata.is_empty());
            let mut expected_owned = [0; 69];
            expected_owned[1..5].copy_from_slice(&16_u32.to_le_bytes());
            expected_owned[5] = 1;
            expected_owned[68] = 128;
            assert_eq!(&after[owned..owned + 69], &expected_owned);
            assert_eq!(append.length, before.len() + 4096);
        }
    }
}

#[test]
fn ordered_unique_bitmap_blocks_and_cancellation_are_atomic_cursor_contracts() {
    let before = source(528);
    let (image, _, _) = construct(&before);
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let declarations = crate::check_table_long_values(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let references = declarations.columns()[0].maps().unwrap();
    for blocks in [&[1, 0][..], &[0, 0][..]] {
        let mut append = State::new(&image, checked.schema().definition_page());
        assert!(matches!(
            append.reserve_long_value_with_owned_bitmaps(
                528,
                2,
                references,
                blocks,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
        assert_eq!(append.reservations_len(), 0);
        assert_eq!(append.length, before.len());
    }
    let mut append = State::new(&image, checked.schema().definition_page());
    let polls = Cell::new(0);
    let error = append.reserve_long_value_with_owned_bitmaps(
        528,
        2,
        references,
        &[0, 1],
        &mut budget(),
        &|| {
            polls.set(polls.get() + 1);
            polls.get() == 4
        },
    );
    assert!(matches!(
        error,
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(append.reservations_len(), 0);
    assert_eq!(append.length, before.len());
    append
        .reserve_long_value_with_owned_bitmaps(528, 2, references, &[0, 1], &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(append.length, before.len() + 3 * 4096);
    assert_eq!(
        append
            .long_values()
            .map(|page| page.number)
            .collect::<Vec<_>>(),
        [528]
    );
    assert_eq!(
        append
            .long_value_owned_bitmaps()
            .map(|page| (page.number, page.block))
            .collect::<Vec<_>>(),
        [(529, 0), (530, 1)]
    );
}

#[test]
fn missing_column_declarations_do_not_become_map_initialization_authority() {
    let mut before = source(528);
    before[10 * 4096 + 500..10 * 4096 + 504].fill(0);
    let image = NativeImage::capture(
        &mut Cursor::new(before.as_slice()),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let profile = Profile::bind(checked.schema(), &mut budget(), &|| false).unwrap();
    let mut inputs = [input::prepare(
        checked.schema(),
        profile.variables(),
        &[
            Value::Int32(10),
            Value::Int32(100),
            Value::Binary(vec![0x92; 4076]),
        ],
        &mut budget(),
        &|| false,
    )
    .unwrap()];
    assert!(
        long_values::prepare(
            &image,
            &checked,
            profile.variables(),
            &mut inputs,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(image.as_bytes(), before);
}

fn captured_checked(before: &[u8]) -> (NativeImage, crate::TableCheck) {
    let image = NativeImage::capture(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    (image, checked)
}

fn prepare_limited(
    image: &NativeImage,
    checked: &crate::TableCheck,
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(long_values::Plan, State), EditError> {
    let profile = Profile::bind(checked.schema(), &mut budget(), &|| false)?;
    let mut inputs = [input::prepare(
        checked.schema(),
        profile.variables(),
        &[
            Value::Int32(10),
            Value::Int32(100),
            Value::Binary(vec![0x92; 4076]),
        ],
        &mut budget(),
        &|| false,
    )?];
    long_values::prepare(
        image,
        checked,
        profile.variables(),
        &mut inputs,
        allowance,
        cancelled,
    )
}

#[test]
fn promotion_checks_each_pool_at_one_short_and_cancellation_at_the_final_poll() {
    let before = source(528);
    let (image, checked) = captured_checked(&before);
    let mut healthy_budget = budget();
    let polls = Cell::new(0_usize);
    let (healthy, append) = prepare_limited(&image, &checked, &mut healthy_budget, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(healthy.metadata.len(), 1);
    assert_eq!(healthy.metadata[0].number, 529);
    assert_eq!(append.reservations_len(), 2);
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let initial = budget().remaining(resource);
        let used = initial - healthy_budget.remaining(resource);
        assert!(used > 0);
        let mut limited = budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let error = prepare_limited(&image, &checked, &mut limited, &|| false).err();
        assert!(matches!(
            error,
            Some(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource
        ));
        assert_eq!(image.as_bytes(), before);
    }
    let observed = Cell::new(0_usize);
    let error = prepare_limited(&image, &checked, &mut budget(), &|| {
        observed.set(observed.get() + 1);
        observed.get() == polls.get()
    })
    .err();
    assert_eq!(error, Some(EditError::Inspect(InspectError::Cancelled)));
    assert_eq!(observed.get(), polls.get());
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn unrepresented_global_future_cells_refuse_instead_of_inventing_map_growth() {
    let mut before = source(528);
    let global = row_start(&before, 1, 0);
    // An inline GLOBAL row still proves all source table/map/chunk pages
    // allocated, but cannot describe either future528 or its bitmap529.
    before[global..global + 69].fill(0);
    let (image, checked) = captured_checked(&before);
    let report = crate::check_table_long_values(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        report.columns()[0].status(),
        crate::LongValueCheckStatus::Checked
    );
    let error = prepare_limited(&image, &checked, &mut budget(), &|| false).err();
    assert_eq!(error, Some(EditError::Inspect(InspectError::Unsupported)));
    assert_eq!(image.as_bytes(), before);
}

#[test]
fn identical_owned_and_hint_bytes_do_not_grant_the_wrong_column_record_authority() {
    let before = source(528);
    let (image, checked) = captured_checked(&before);
    let inventory = Inventory::capture(&image, &mut budget(), &|| false).unwrap();
    let owned = row_start(&before, 11, 2);
    let candidate = row_start(&before, 11, 3);
    assert_eq!(
        &before[owned..owned + 69],
        &before[candidate..candidate + 69]
    );
    inventory
        .require_record_rewrite(
            checked.schema(),
            2,
            ColumnMapKind::Owned,
            ((11 << 8) | 2, owned..owned + 69),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        inventory.require_record_rewrite(
            checked.schema(),
            2,
            ColumnMapKind::Owned,
            ((11 << 8) | 3, candidate..candidate + 69),
            &mut budget(),
            &|| false,
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    let mut aliased = before.clone();
    aliased[10 * 4096 + 504..10 * 4096 + 508].copy_from_slice(&((11_u32 << 8) | 2).to_le_bytes());
    let (alias_image, alias_checked) = captured_checked(&aliased);
    assert!(matches!(
        prepare_limited(&alias_image, &alias_checked, &mut budget(), &|| false),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
    assert_eq!(alias_image.as_bytes(), aliased);
}
