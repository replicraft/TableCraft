//! Whole independently authored AFTER and bounded append/source refusals.
mod repeated;
use super::{AddTableError, AddTablePlan, append::Page, audit, source};
use crate::{CreateColumn, FirstTableDefinition, NativeImage, read_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Double, Single, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::{
    canonical_add_table as fixtures, canonical_blank_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(492);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn definition(name: &str) -> FirstTableDefinition {
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Name", 20).unwrap(),
    ];
    FirstTableDefinition::capture(name, &columns, &mut budget(), &|| false).unwrap()
}

fn plan(bytes: &[u8]) -> AddTablePlan {
    image(bytes)
        .plan_add_table(definition(fixtures::NEW_TABLE), &mut budget(), &|| false)
        .unwrap()
}

#[test]
fn append_matches_independent_complete_image_and_keeps_existing_rows_roots_and_opaque_gaps() {
    let fixture = fixtures::source();
    let plan = plan(&fixture.before);
    assert_eq!(plan.before_bytes(), fixture.before);
    assert_eq!(plan.after_bytes(), fixture.after);
    assert_eq!(plan.table(), fixtures::NEW_TABLE);
    assert_eq!(plan.column_count(), 2);
    assert_eq!(
        &plan.after_bytes()[7 * 4096..10 * 4096],
        &fixture.before[7 * 4096..10 * 4096]
    );
    let after = image(plan.after_bytes());
    let old = read_table(
        &mut after.reader(),
        SCOPE,
        fixtures::FIRST_TABLE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(old.rows(), &[fixture.original_row]);
    let new = read_table(
        &mut after.reader(),
        SCOPE,
        fixtures::NEW_TABLE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(new.rows().is_empty());
    assert_eq!(new.schema().columns()[1].storage_length(), 40);
}

#[test]
fn conflicting_names_nonascii_ambiguity_and_live_eof_claims_refuse_without_candidate() {
    let fixture = fixtures::source();
    for name in [fixtures::FIRST_TABLE, "items"] {
        assert!(matches!(
            image(&fixture.before).plan_add_table(definition(name), &mut budget(), &|| false),
            Err(AddTableError::DuplicateName)
        ));
    }
    assert!(matches!(
        image(&fixture.before).plan_add_table(definition("雪"), &mut budget(), &|| false),
        Err(AddTableError::Source)
    ));
    for indexed in [false, true] {
        assert!(matches!(
            image(&fixtures::claimed_append_destination(indexed)).plan_add_table(
                definition(fixtures::NEW_TABLE),
                &mut budget(),
                &|| false
            ),
            Err(AddTableError::Source)
        ));
    }
    assert!(matches!(
        image(&canonical_blank_creation::source()).plan_add_table(
            definition(fixtures::NEW_TABLE),
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Source)
    ));
}

#[test]
fn independent_audit_refuses_changes_to_existing_opaque_bytes_counts_maps_and_new_declarations() {
    let fixture = fixtures::source();
    let before = image(&fixture.before);
    let source = source::capture(&before, fixtures::NEW_TABLE, &mut budget(), &|| false).unwrap();
    for (at, bit) in [
        (7 * 4096 + 3500, 1),
        (3 * 4096 + 512, 1),
        (4 * 4096 + 512, 1),
        (8 * 4096 + 4090, 1),
        (9 * 4096 + 484, 1),
        (2 * 4096 + 16, 1),
        (10 * 4096 + 75 + 25 + 23, 1),
        (10 * 4096 + 75 + 21, 1),
        (10 * 4096 + 75 + 25 + 7, 1),
        (10 * 4096 + 75 + 25 + 11, 1),
        (10 * 4096 + 3500, 1),
        (11 * 4096 + 512, 1),
        (12 * 4096 + 3500, 1),
        (12 * 4096 + 4, 1),
    ] {
        let mut forged = fixture.after.clone();
        forged[at] ^= bit;
        assert!(
            audit::verify(
                &before,
                &forged,
                &definition(fixtures::NEW_TABLE),
                &source,
                &mut budget(),
                &|| false
            )
            .is_err(),
            "offset {at}"
        );
    }
    // The candidate map is not selected by normal row reads, so audit checks it directly.
    let maps = Page::capture(source::page(&fixture.after, 3), 0, &mut budget(), &|| false).unwrap();
    let mut forged = fixture.after.clone();
    forged[3 * 4096 + maps.records[24].start + 6] ^= 1 << 3;
    assert!(
        audit::verify(
            &before,
            &forged,
            &definition(fixtures::NEW_TABLE),
            &source,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn append_capacity_refusal_is_atomic_and_existing_directory_offsets_do_not_move() {
    let fixture = fixtures::source();
    let mut page = source::page(&fixture.before, 4).to_vec();
    let mut layout = Page::capture(&page, 2, &mut budget(), &|| false).unwrap();
    let original = page.clone();
    assert!(matches!(
        layout.append(&mut page, 4096),
        Err(AddTableError::Capacity)
    ));
    assert_eq!(page, original);
    assert_eq!(layout.count, 8);
    let old_directory = page[14..30].to_vec();
    layout.append(&mut page, 20).unwrap();
    assert_eq!(&page[14..30], old_directory);
    for range in &layout.records {
        assert_eq!(&page[range.clone()], &original[range.clone()]);
    }
}

#[test]
fn defaults_and_policies_are_decoded_independently_with_exact_requested_carriers() {
    let fixture = fixtures::source();
    let values = [
        Value::Byte(255),
        Value::Int16(-7),
        Value::Single(Single::from_bits(1)),
        Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
        Value::Decimal(Decimal::new(123450, 2).unwrap()),
        Value::Text(String::new()),
    ];
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Byte", ScalarType::Byte)
            .with_required(false)
            .with_default_literal(&values[0])
            .unwrap(),
        CreateColumn::new("Short", ScalarType::Int16)
            .with_default_literal(&values[1])
            .unwrap(),
        CreateColumn::new("Single", ScalarType::Single)
            .with_default_literal(&values[2])
            .unwrap(),
        CreateColumn::new("Double", ScalarType::Double)
            .with_default_literal(&values[3])
            .unwrap(),
        CreateColumn::decimal("Decimal", DecimalConstraints::new(6, 2).unwrap())
            .with_default_literal(&values[4])
            .unwrap(),
        CreateColumn::short_text("Name", 20)
            .unwrap()
            .with_required(true)
            .with_allow_zero_length(true)
            .unwrap()
            .with_default_literal(&values[5])
            .unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture(fixtures::NEW_TABLE, &columns, &mut budget(), &|| false)
            .unwrap();
    let plan = image(&fixture.before)
        .plan_add_table(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        &plan.after_bytes()[7 * 4096..10 * 4096],
        &fixture.before[7 * 4096..10 * 4096]
    );
    // Same-width replacement remains a well-formed expression, but no longer matches the request.
    let mut forged = plan.after_bytes().to_vec();
    let needle = [b'2', 0, b'5', 0, b'5', 0];
    let at = forged[4 * 4096..5 * 4096]
        .windows(needle.len())
        .position(|bytes| bytes == needle)
        .unwrap()
        + 4 * 4096;
    forged[at] = b'1';
    let source =
        source::capture(&plan.before, fixtures::NEW_TABLE, &mut budget(), &|| false).unwrap();
    assert!(
        audit::verify(
            &plan.before,
            &forged,
            &plan.definition,
            &source,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn complete_addition_accounts_exact_pools_and_cancels_before_candidate_publication() {
    let fixture = fixtures::source();
    let invoke = |work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool| {
        image(&fixture.before)
            .plan_add_table(definition(fixtures::NEW_TABLE), work, cancelled)
            .map(|_| ())
    };
    let mut measured = budget();
    let initial = POOLS.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0);
    invoke(&mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let used: [u64; 3] =
        std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
    invoke(&mut ResourceBudget::new(used[0], used[1], used[2]), &|| {
        false
    })
    .unwrap();
    for missing in 0..3 {
        let mut short = used;
        short[missing] -= 1;
        assert!(
            invoke(
                &mut ResourceBudget::new(short[0], short[1], short[2]),
                &|| false
            )
            .is_err()
        );
    }
    for limit in [0, polls.get() / 2, polls.get() - 1] {
        let calls = Cell::new(0);
        assert!(
            invoke(&mut budget(), &|| {
                let at = calls.get();
                calls.set(at + 1);
                at >= limit
            })
            .is_err()
        );
    }
}

#[test]
fn valid_catalog_name_cannot_expand_its_record_to_claim_and_erase_an_old_opaque_gap() {
    let fixture = fixtures::source();
    let before = image(&fixture.before);
    let source = source::capture(&before, fixtures::NEW_TABLE, &mut budget(), &|| false).unwrap();
    let catalog =
        Page::capture(source::page(&fixture.after, 4), 2, &mut budget(), &|| false).unwrap();
    let range = catalog.records[8].clone();
    let raw = &source::page(&fixture.after, 4)[range.clone()];
    assert_eq!(raw.len(), 69);
    let start = 500;
    let padding = range.start - start;
    let mut expanded = raw[..40].to_vec();
    expanded.resize(40 + padding, 0);
    let end = expanded.len();
    expanded.extend((end as u16).to_le_bytes());
    for slot in (0..11).rev() {
        // Owner is NULL. Its ignored span can hide padding without changing
        // the decoded Name; all later absent carriers stay at EOD.
        let offset = match slot {
            0 => 32,
            1 => 40,
            _ => end,
        };
        expanded.extend((offset as u16).to_le_bytes());
    }
    expanded.extend([11, 0, 0xbf, 0, 0]);
    assert_eq!(expanded.len(), range.len() + padding);
    let mut forged = fixture.after;
    let page = &mut forged[4 * 4096..5 * 4096];
    page[start..range.end].copy_from_slice(&expanded);
    page[30..32].copy_from_slice(&(start as u16).to_le_bytes());
    page[2..4].copy_from_slice(&((start - 32) as u16).to_le_bytes());
    let decoded = image(&forged);
    assert!(
        decoded
            .catalog()
            .catalog()
            .lookup(fixtures::NEW_TABLE, None, None)
            .unwrap()
            .is_some()
    );
    assert_ne!(
        &forged[4 * 4096 + 512..4 * 4096 + 528],
        &fixture.before[4 * 4096 + 512..4 * 4096 + 528]
    );
    assert!(matches!(
        audit::verify(
            &before,
            &forged,
            &definition(fixtures::NEW_TABLE),
            &source,
            &mut budget(),
            &|| false
        ),
        Err(AddTableError::Preservation)
    ));
}
