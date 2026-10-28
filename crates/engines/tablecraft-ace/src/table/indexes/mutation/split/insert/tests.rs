use super::*;
use crate::table::indexes::mutation::{NumericKey, profile::SingleProfile};
use crate::{IndexCheckStatus, IndexConstraintViolation, check_table, check_table_constraints};
use model::{PAGE, ROOT};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

mod model;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
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

fn checked(bytes: &[u8], count: usize, aliases: usize) -> TableCheck {
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), count);
    assert_eq!(checked.indexes().len(), aliases);
    assert!(
        checked
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
    assert_eq!(constraints.rows(), count);
    checked
}

fn seeded_data(image: &NativeImage, number: u32) -> State {
    let mut state = State::new(image, 10);
    let mut source = image.reader();
    let mut allowance = budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut allowance,
        cancelled: &|| false,
        reads: 0,
    };
    for page in image.as_bytes().len() as u32 / 4096..=number {
        state
            .reserve_data(page, reader.budget, reader.cancelled)
            .unwrap();
        for (reference, role, set) in [
            (11 << 8, MapRole::Owned, true),
            ((11 << 8) | 1, MapRole::TableFreeSpace, true),
            (GLOBAL_FREE_REFERENCE, MapRole::GlobalFree, false),
        ] {
            let map = RepresentedMap::read(&mut reader, reference, role).unwrap();
            let cell = map.future_cell(&mut reader, page).unwrap();
            append::claim(&mut state.map_edits, cell, set, &mut reader).unwrap();
        }
    }
    state
}

#[test]
fn exact451_to452_overflow_retains_root_and_physical_aliases() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        for aliases in [false, true] {
            let (before, original) = model::source(code, 451, aliases, false);
            let structural = checked(&before, 451, if aliases { 2 } else { 1 });
            let image = capture(&before);
            let reference = (15 << 8) | 196;
            let keys = model::final_keys(&original, reference);
            assert_eq!(
                u16::from_le_bytes(before[13 * 4096 + 2..13 * 4096 + 4].try_into().unwrap()),
                7,
            );
            assert_eq!(keys.len() * 9 - (keys.len() - 1), 3617);
            let plan = continue_index::<_, ENTRY_BUFFER>(
                Operation {
                    image: &image,
                    checked: &structural,
                    index: structural.indexes()[0].index(),
                    profile: &profile,
                    final_keys: &keys,
                },
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
                [16, 17]
            );
            assert!(plan.promotions.is_empty());
            assert!(plan.rebases.is_empty());
            let expected = model::expected(&before, &keys, reference, false, false);
            let actual = model::compose(&before, &plan, reference);
            model::assert_image(&actual, &expected);
            checked(&actual, 452, if aliases { 2 } else { 1 });
        }
    }
}

#[test]
fn prior_data_reservation_and_index_children_share_one_cursor_and_global_byte() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        let (before, original) = model::source(code, 451, true, false);
        let structural = checked(&before, 451, 2);
        let image = capture(&before);
        let reference = 16 << 8;
        let keys = model::final_keys(&original, reference);
        let state = seeded_data(&image, 16);
        let plan = continue_index::<_, ENTRY_BUFFER>(
            Operation {
                image: &image,
                checked: &structural,
                index: structural.indexes()[0].index(),
                profile: &profile,
                final_keys: &keys,
            },
            state,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        plan.append
            .require_data(16, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            plan.added
                .iter()
                .map(|page| page.number)
                .collect::<Vec<_>>(),
            [17, 18]
        );
        assert_eq!(plan.append.length, 19 * 4096);
        let actual = model::compose(&before, &plan, reference);
        model::assert_image(
            &actual,
            &model::expected(&before, &keys, reference, true, false),
        );
        checked(&actual, 452, 2);
    }
}

#[test]
fn indirect_promotion_reserves_distinct_bitmap_between_the_two_index_leaves() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for code in [5, 6] {
        let (before, original) = model::source(code, 451, true, true);
        let structural = checked(&before, 451, 2);
        let image = capture(&before);
        let reference = (15 << 8) | 196;
        let keys = model::final_keys(&original, reference);
        let plan = continue_index::<_, ENTRY_BUFFER>(
            Operation {
                image: &image,
                checked: &structural,
                index: structural.indexes()[0].index(),
                profile: &profile,
                final_keys: &keys,
            },
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
            [600, 601, 602]
        );
        assert_eq!(plan.promotions.len(), 1);
        assert_eq!(plan.promotions[0].number, 601);
        plan.append
            .require_index_bitmap(601, 0, 14 << 8, &mut budget(), &|| false)
            .unwrap();
        let actual = model::compose(&before, &plan, reference);
        model::assert_image(
            &actual,
            &model::expected(&before, &keys, reference, false, true),
        );
        checked(&actual, 452, 2);
    }
}

#[test]
fn fitting_root_refuses_growth_while_height1_uses_the_general_continuation() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let (before, original) = model::source(6, 450, false, false);
    let checked_source = checked(&before, 450, 1);
    let image = capture(&before);
    let keys = model::final_keys(&original, (15 << 8) | 195);
    let error = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &checked_source,
            index: checked_source.indexes()[0].index(),
            profile: &profile,
            final_keys: &keys,
        },
        State::new(&image, 10),
        &mut budget(),
        &|| false,
    )
    .err()
    .unwrap();
    assert_eq!(error, EditError::Unsupported(WriteBlocker::IndexedColumn));

    let (before, original) = model::source(6, 451, false, false);
    let reference = (15 << 8) | 196;
    let keys = model::final_keys(&original, reference);
    let grown = model::expected(&before, &keys, reference, false, false);
    let checked_source = checked(&grown, 452, 1);
    let image = capture(&grown);
    let mut second = keys.clone();
    second.push(KeyRef {
        key: Some(-1),
        row: (15 << 8) | 197,
    });
    second.sort_by_key(|entry| (entry.key, entry.row));
    let plan = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &checked_source,
            index: checked_source.indexes()[0].index(),
            profile: &profile,
            final_keys: &second,
        },
        State::new(&image, 10),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(plan.added.is_empty());
    assert!(plan.promotions.is_empty());
    assert!(plan.rebases.is_empty());
    assert_eq!(plan.append.length, grown.len());
    assert_eq!(plan.patches.len(), 1);
    let left = u32::from_be_bytes(
        grown[ROOT * PAGE + 489..ROOT * PAGE + 493]
            .try_into()
            .unwrap(),
    );
    let patch = &plan.patches[0];
    assert_eq!(patch.offset, left as usize * PAGE);
    assert_eq!(patch.after[2..4], 1573_u16.to_le_bytes());
    assert_eq!(patch.after[24..26], [0, 0]);
}

#[test]
fn changed_original_keys_duplicate_refs_and_duplicate_primary_values_refuse() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let (before, original) = model::source(6, 451, false, false);
    let structural = checked(&before, 451, 1);
    let image = capture(&before);
    let keys = model::final_keys(&original, (15 << 8) | 196);
    for case in 0..3 {
        let mut forged = keys.clone();
        let expected = match case {
            0 => {
                forged[0].key = Some(i64::from(i32::MIN) + 1);
                EditError::Conflict
            }
            1 => {
                forged[1].row = original.last().unwrap().row;
                EditError::Conflict
            }
            _ => {
                forged[1].key = forged[0].key;
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey,
                ))
            }
        };
        let error = continue_index::<_, ENTRY_BUFFER>(
            Operation {
                image: &image,
                checked: &structural,
                index: structural.indexes()[0].index(),
                profile: &profile,
                final_keys: &forged,
            },
            State::new(&image, 10),
            &mut budget(),
            &|| false,
        )
        .err()
        .unwrap();
        assert_eq!(error, expected);
    }
}

#[test]
fn foreign_role_alias_and_allocated_future_cells_refuse_without_a_plan() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    for role_alias in [false, true] {
        let (mut before, original) = model::source(6, 451, false, false);
        if role_alias {
            support::put32(&mut before, 10 * 4096 + 59, 14 << 8);
        } else {
            before[4096 + 4027 + 5 + 16 / 8] &= !(1 << (16 % 8));
        }
        let structural = checked(&before, 451, 1);
        let image = capture(&before);
        let keys = model::final_keys(&original, (15 << 8) | 196);
        let error = continue_index::<_, ENTRY_BUFFER>(
            Operation {
                image: &image,
                checked: &structural,
                index: structural.indexes()[0].index(),
                profile: &profile,
                final_keys: &keys,
            },
            State::new(&image, 10),
            &mut budget(),
            &|| false,
        )
        .err()
        .unwrap();
        let expected = if role_alias {
            EditError::Unsupported(WriteBlocker::RowLayout)
        } else {
            EditError::Inspect(InspectError::Malformed)
        };
        assert_eq!(error, expected);
    }
}

#[test]
fn bounded_work_and_late_cancellation_leave_retry_images_deterministic() {
    let profile = SingleProfile {
        column: 0,
        kind: NumericKey::new(tablecraft_schema::ScalarType::Int32).unwrap(),
        descending: false,
    };
    let (before, original) = model::source(6, 451, true, false);
    let structural = checked(&before, 451, 2);
    let image = capture(&before);
    let reference = (15 << 8) | 196;
    let keys = model::final_keys(&original, reference);
    for (mut allowance, resource) in [
        (
            ResourceBudget::new(0, 1_000_000, 100_000_000),
            Resource::Bytes,
        ),
        (
            ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 0),
            Resource::Steps,
        ),
    ] {
        let error = continue_index::<_, ENTRY_BUFFER>(
            Operation {
                image: &image,
                checked: &structural,
                index: structural.indexes()[0].index(),
                profile: &profile,
                final_keys: &keys,
            },
            State::new(&image, 10),
            &mut allowance,
            &|| false,
        )
        .err()
        .unwrap();
        assert!(
            matches!(error, EditError::Inspect(InspectError::Limit(limit)) if limit.resource == resource)
        );
    }
    let calls = std::cell::Cell::new(0);
    let plan = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &structural,
            index: structural.indexes()[0].index(),
            profile: &profile,
            final_keys: &keys,
        },
        State::new(&image, 10),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let expected = model::expected(&before, &keys, reference, false, false);
    model::assert_image(&model::compose(&before, &plan, reference), &expected);
    let stop = calls.get() - 1;
    let current = std::cell::Cell::new(0);
    let error = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &structural,
            index: structural.indexes()[0].index(),
            profile: &profile,
            final_keys: &keys,
        },
        State::new(&image, 10),
        &mut budget(),
        &|| {
            current.set(current.get() + 1);
            current.get() == stop
        },
    )
    .err()
    .unwrap();
    assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
    let retry = continue_index::<_, ENTRY_BUFFER>(
        Operation {
            image: &image,
            checked: &structural,
            index: structural.indexes()[0].index(),
            profile: &profile,
            final_keys: &keys,
        },
        State::new(&image, 10),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    model::assert_image(&model::compose(&before, &retry, reference), &expected);
    model::assert_image(image.as_bytes(), &before);
}
