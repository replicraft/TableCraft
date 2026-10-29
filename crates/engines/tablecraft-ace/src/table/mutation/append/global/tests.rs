//! Actual typed reservations distinguish purpose pages from global metadata.
use crate::{
    EditError, IndexCheckStatus, InspectError, LongValueMapReferences, NativeImage, check_table,
    check_table_long_values,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    inspect_table_schema,
    table::mutation::{
        append::State,
        insert::tests::global_map_growth::model::{self, Phase},
        long_value::ownership::Inventory,
    },
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn captured(bytes: &[u8], configured: bool) -> (NativeImage, State, LongValueMapReferences) {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let report = check_table_long_values(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let references = report.columns()[0].maps().unwrap();
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut append = State::new(&image, schema.definition_page());
    if configured {
        let inventory = Inventory::capture(&image, &mut budget(), &|| false).unwrap();
        let mut input = image.reader();
        let mut resources = budget();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget: &mut resources,
            cancelled: &|| false,
            reads: 0,
        };
        append
            .enable_global_growth(&mut reader, |record, reader| {
                inventory.require_global_record_rewrite(record, reader)
            })
            .unwrap();
    }
    (image, append, references)
}

fn claim(image: &NativeImage, append: &mut State, numbers: &[u32]) {
    let mut input = image.reader();
    let mut resources = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    let global =
        RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree).unwrap();
    for &number in numbers {
        append.claim_global(&global, number, &mut reader).unwrap();
    }
}

fn prepared(code: u8) -> (NativeImage, State, LongValueMapReferences) {
    let case = model::case(code, Phase::OwnedBitmap, 512, true);
    let (image, mut append, references) = captured(&case.before, true);
    assert_eq!(append.planned_number(0).unwrap(), 512);
    assert_eq!(append.planned_number(1).unwrap(), 514);
    assert_eq!(append.preview(2).unwrap(), 515 * PAGE);
    assert!(append.global_record().is_none());
    claim(&image, &mut append, &[512, 514]);
    (image, append, references)
}

#[test]
fn owned_body_and_bitmap_return_actual_ids_around_the_closed_global_reservation() {
    for code in [5, 6] {
        let (image, mut append, references) = prepared(code);
        let actual = append
            .reserve_long_value_with_owned_bitmaps(512, 2, references, &[0], &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(actual, [512, 514]);
        assert_eq!(append.length, 515 * PAGE);
        assert_eq!(append.next().unwrap(), 515);
        assert_eq!(append.reservations_len(), 3);
        assert_eq!(append.global_bitmaps().collect::<Vec<_>>(), [(513, 256, 0)]);
        assert_eq!(
            append
                .long_values()
                .map(|page| page.number)
                .collect::<Vec<_>>(),
            [512]
        );
        assert_eq!(
            append
                .long_value_owned_bitmaps()
                .map(|page| page.number)
                .collect::<Vec<_>>(),
            [514]
        );
        append
            .require_long_value(
                512,
                2,
                references.owned(),
                references.candidate(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        append
            .require_long_value_owned_bitmap(514, 2, references.owned(), 0, &mut budget(), &|| {
                false
            })
            .unwrap();
        append
            .require_global_bitmap(513, 256, 0, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            append.require_data(513, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        assert_eq!(
            append.require_global_bitmap(514, 256, 0, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        assert_eq!(
            append.require_global_bitmap(513, 512, 0, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        assert_eq!(
            append.require_global_bitmap(513, 256, 1, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
        let record = append.global_record().unwrap();
        assert_eq!(
            record.before,
            image.as_bytes()[record.offset..record.offset + 69]
        );
        assert_eq!(&record.after[..5], &[1, 1, 2, 0, 0]);
        assert!(record.after[5..].iter().all(|byte| *byte == 0));
        let bitmap = append.global_bitmap().unwrap();
        assert_eq!(&bitmap.bytes[..4], &[5, 1, 0, 0]);
        assert_eq!(bitmap.bytes[68], 248); // Body512, global513, owned514 are used.
        assert_eq!(&bitmap.bytes[4092..], &[0; 4]);
        assert_eq!(bitmap.bytes[4 + 30 / 8] & (1 << (30 % 8)), 0);
    }
}

#[test]
fn invalid_owned_block_groups_do_not_commit_any_half_of_the_coallocation() {
    for blocks in [&[0, 0][..], &[1, 0][..]] {
        let (_, mut append, references) = prepared(6);
        let record = append.global_record().unwrap().after.clone();
        let bitmap = append.global_bitmap().unwrap().bytes.clone();
        assert_eq!(
            append.reserve_long_value_with_owned_bitmaps(
                512,
                2,
                references,
                blocks,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        );
        assert_eq!(append.length, 512 * PAGE);
        assert_eq!(append.reservations_len(), 0);
        assert_eq!(append.global_bitmaps().count(), 0);
        assert_eq!(append.global_record().unwrap().after, record);
        assert_eq!(append.global_bitmap().unwrap().bytes, bitmap);
        assert_eq!(
            append
                .reserve_long_value_with_owned_bitmaps(
                    512,
                    2,
                    references,
                    &[0],
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            [512, 514]
        );
    }
}

#[test]
fn reservation_limits_and_terminal_cancellation_leave_the_planned_global_prefix_uncommitted() {
    let (_, mut healthy, references) = prepared(6);
    let polls = Cell::new(0_usize);
    let mut used_budget = budget();
    assert_eq!(
        healthy
            .reserve_long_value_with_owned_bitmaps(
                512,
                2,
                references,
                &[0],
                &mut used_budget,
                &|| {
                    polls.set(polls.get() + 1);
                    false
                },
            )
            .unwrap(),
        [512, 514]
    );
    for resource in [Resource::Bytes, Resource::Steps] {
        let (_, mut append, references) = prepared(6);
        let record = append.global_record().unwrap().after.clone();
        let bitmap = append.global_bitmap().unwrap().bytes.clone();
        let initial = budget().remaining(resource);
        let used = initial - used_budget.remaining(resource);
        assert!(used > 0);
        let mut limited = budget();
        limited.charge(resource, initial - (used - 1)).unwrap();
        let result = append.reserve_long_value_with_owned_bitmaps(
            512,
            2,
            references,
            &[0],
            &mut limited,
            &|| false,
        );
        assert!(
            matches!(result, Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource)
        );
        assert_eq!(append.length, 512 * PAGE);
        assert_eq!(append.reservations_len(), 0);
        assert_eq!(append.global_bitmaps().count(), 0);
        assert_eq!(append.global_record().unwrap().after, record);
        assert_eq!(append.global_bitmap().unwrap().bytes, bitmap);
    }
    let (_, mut append, references) = prepared(6);
    let observed = Cell::new(0_usize);
    assert_eq!(
        append.reserve_long_value_with_owned_bitmaps(
            512,
            2,
            references,
            &[0],
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                observed.get() == polls.get()
            },
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    );
    assert_eq!(append.length, 512 * PAGE);
    assert_eq!(append.reservations_len(), 0);
    assert_eq!(append.global_bitmaps().count(), 0);
    // Claim preparation predates this reservation: its private intent remains,
    // while no closed purpose, cursor advance or candidate has been published.
    assert!(append.global_bitmap().is_some());
}

#[test]
fn unconfigured_state_retains_represented_only_update_allocation() {
    let case = model::case(6, Phase::OwnedBitmap, 512, false);
    let (image, mut append, _) = captured(&case.before, false);
    assert_eq!(append.planned_number(0).unwrap(), 512);
    assert_eq!(append.planned_number(1).unwrap(), 513);
    assert_eq!(append.preview(2).unwrap(), 514 * PAGE);
    let mut input = image.reader();
    let mut resources = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut resources,
        cancelled: &|| false,
        reads: 0,
    };
    let original =
        RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree).unwrap();
    assert_eq!(
        append.claim_global(&original, 512, &mut reader),
        Err(EditError::Inspect(InspectError::Unsupported))
    );
    assert_eq!(append.length, 512 * PAGE);
    assert_eq!(append.reservations_len(), 0);
    assert!(append.global_record().is_none());
    assert!(append.global_bitmap().is_none());
}

#[test]
fn a_literal_indirect_next_source_keeps_its_existing_global_storage_and_cells() {
    let case = model::case(6, Phase::OwnedBitmap, 512, true);
    let (image, mut append, _) = captured(&case.expected, true);
    assert_eq!(append.next().unwrap(), 515);
    assert_eq!(append.planned_number(1).unwrap(), 516);
    claim(&image, &mut append, &[515]);
    append.reserve_data(515, &mut budget(), &|| false).unwrap();
    assert_eq!(append.length, 516 * PAGE);
    assert_eq!(append.reservations_len(), 1);
    assert!(append.global_record().is_none());
    assert!(append.global_bitmap().is_none());
    assert_eq!(append.global_bitmaps().count(), 0);
    assert_eq!(append.map_edits.len(), 1);
    assert_eq!(append.map_edits[0].offset, 513 * PAGE + 68);
    assert_eq!(append.map_edits[0].before, 248);
    assert_eq!(append.map_edits[0].after, 240);
}
