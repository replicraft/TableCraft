#[cfg(feature = "ace16-write-preview")]
use super::encoded_model;
use super::model::*;
use std::{cell::Cell, io::Cursor};
#[cfg(feature = "ace16-write-preview")]
use tablecraft_ace::{EditError, Int32Update, WriteBlocker};
use tablecraft_ace::{
    GeneralEqualityPrefix, GeneralTextSearch, IndexCheckStatus, InspectError, NativeSortOrder,
    TextIndexSelection, check_table, compare_general_like_units, compare_general_text,
    find_general_text, read_table_index, read_table_text_index_range,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[test]
fn french_reads_do_not_activate_general_comparison_hints_or_any_mutations() {
    let model = database(6, false, None);
    let order = NativeSortOrder::from_native_bytes([12, 4, 0, 0]);
    assert_eq!(order.general_family(), None);
    assert!(matches!(
        compare_general_text(order, "côte", "coté", &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        compare_general_like_units(order, &[97], &[97], &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        find_general_text(
            order,
            "côte",
            "o",
            GeneralTextSearch::ForwardFrom(0),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        GeneralEqualityPrefix::from_text(order, "cote", &mut budget(), &|| false),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        read_table_text_index_range(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            TextIndexSelection::EqualityCandidates("cote"),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    #[cfg(feature = "ace16-write-preview")]
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_insert_rows("Items", &[], &mut budget(), &|| false)
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
    #[cfg(feature = "ace16-write-preview")]
    for deleted in [vec![], vec![0], vec![4]] {
        assert_eq!(
            candidate(&model, &deleted).err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
    #[cfg(feature = "ace16-write-preview")]
    for updates in [vec![], vec![Int32Update::new(0, 1, 2)]] {
        assert!(matches!(
            capture(&model.bytes).preview_ace16_int32_updates(
                "Items",
                "Id",
                &updates,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(_))
        ));
    }
}

#[test]
fn unknown_descriptors_memo_and_tuples_retain_explicit_unsupported_status() {
    let mut fixtures = Vec::new();
    for raw in [[12, 4, 1, 0], [12, 4, 0, 1], [12, 8, 0, 0]] {
        let mut model = database(6, false, None);
        model.bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&raw);
        fixtures.push(model.bytes);
    }
    #[cfg(feature = "ace16-write-preview")]
    {
        let mut memo = encoded_model::null_memo();
        memo.bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[12, 4, 0, 0]);
        fixtures.push(memo.bytes);
        let mut tuple = encoded_model::composite();
        tuple.bytes[10 * 4096 + 111..10 * 4096 + 115].copy_from_slice(&[12, 4, 0, 0]);
        fixtures.push(tuple.bytes);
    }
    for bytes in fixtures {
        let result = check_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            result
                .indexes()
                .iter()
                .any(|index| index.status() == IndexCheckStatus::Unsupported)
        );
        assert!(matches!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn selected_french_reads_validate_full_keys_owner_and_hidden_slots() {
    let original = database(6, false, None);
    read_table_index(
        &mut Cursor::new(&original.bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for (offset, replacement) in [(13 * 4096 + 4, 11), (13 * 4096 + 480, 127)] {
        let mut model = database(6, false, None);
        model.bytes[offset] = replacement;
        assert!(
            read_table_index(
                &mut Cursor::new(&model.bytes),
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
fn cumulative_limits_and_observed_cancellation_refuse_without_modifying_source() {
    let model = database(6, false, Some(false));
    let calls = Cell::new(0);
    read_table_index(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for at in [1, calls.get() / 2, calls.get()] {
        let observed = Cell::new(0);
        let mut source = Cursor::new(model.bytes.clone());
        let result = read_table_index(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                observed.get() == at
            },
        );
        assert!(matches!(result, Err(InspectError::Cancelled)));
        assert_eq!(source.into_inner(), model.bytes);
    }
    for mut limited in [
        ResourceBudget::new(1, 100_000, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 1, 30_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 1),
    ] {
        assert!(matches!(
            read_table_index(
                &mut Cursor::new(&model.bytes),
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn unindexed_french_field_does_not_block_unchanged_numeric_write_admission() {
    let mut model = database(6, false, None);
    model.bytes[10 * 4096 + 147..10 * 4096 + 149].copy_from_slice(&0_u16.to_le_bytes());
    model.order = (0..9).collect();
    model.keys = (1_i32..=9)
        .map(|value| {
            let mut key = vec![0x7f];
            key.extend((value as u32 ^ 0x8000_0000).to_be_bytes());
            key
        })
        .collect();
    let entries = model
        .order
        .iter()
        .map(|&row| super::model::wire(&model, row))
        .collect::<Vec<_>>();
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&super::model::page(0, &entries, 0, 0, 0));
    let checked = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
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
    let candidate = capture(&model.bytes)
        .preview_ace16_int32_updates("Items", "Id", &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.candidate_bytes(), model.bytes);
}

#[test]
fn ignored_french_membership_stays_outside_the_ordinary_flag_profile() {
    let mut model = database(6, false, None);
    let original = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        original
            .indexes()
            .iter()
            .all(|item| item.status() == IndexCheckStatus::Checked)
    );

    model.bytes[10 * 4096 + 189] = 0x82;
    let entries = model
        .order
        .iter()
        .copied()
        .filter(|row| *row != 0)
        .map(|row| super::model::wire(&model, row))
        .collect::<Vec<_>>();
    model.bytes[13 * 4096..14 * 4096].copy_from_slice(&super::model::page(0, &entries, 0, 0, 0));
    let checked = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        checked
            .indexes()
            .iter()
            .all(|item| item.status() == IndexCheckStatus::Unsupported)
    );
    assert!(matches!(
        read_table_index(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported)
    ));

    #[cfg(feature = "ace16-write-preview")]
    {
        let mut general = encoded_model::text(0, false, true);
        general.bytes[10 * 4096 + 189] = 0x82;
        let entries = general
            .order
            .iter()
            .copied()
            .filter(|row| *row != 0)
            .map(|row| encoded_model::wire(&general, row))
            .collect::<Vec<_>>();
        general.bytes[13 * 4096..14 * 4096]
            .copy_from_slice(&encoded_model::page(0, &entries, 0, 0, 0));
        let checked = check_table(
            &mut Cursor::new(&general.bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|item| item.status() == IndexCheckStatus::Checked)
        );
    }
}
