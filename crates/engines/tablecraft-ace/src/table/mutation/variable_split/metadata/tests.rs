//! Literal metadata protection checks; these do not qualify a scalar writer.
use super::Metadata;
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker, check_table,
    table::mutation::map_authority::Owner,
    test_support::{map_growth::model, support},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8) -> model::Model {
    let mut original = model::source(code);
    let definition = 10 * 4096;
    assert_eq!(model::number(&original.bytes, definition + 59, 4), 0);
    assert_eq!(model::number(&original.bytes, definition + 55, 4), 11 << 8);
    let owned_range = model::row_range(&original.bytes, 11, 0);
    let owned = original.bytes[owned_range.0..owned_range.1].to_vec();
    let owner = model::number(&original.bytes, 11 * 4096 + 4, 4) as u32;
    let container = support::data_page(owner, &[(0, owned.clone()), (0, model::map(&[]))]);
    model::install(&mut original.bytes, 11, &container);
    support::put32(&mut original.bytes, definition + 59, (11 << 8) | 1);

    assert_eq!(model::row_range(&original.bytes, 11, 0), owned_range);
    assert_eq!(&original.bytes[owned_range.0..owned_range.1], owned);
    assert_eq!(model::number(&original.bytes, definition + 55, 4), 11 << 8);
    assert_eq!(
        model::number(&original.bytes, 11 * 4096 + 4, 4),
        owner as usize
    );
    assert_eq!(
        model::number(&original.bytes, definition + 59, 4),
        (11 << 8) | 1
    );
    original
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

fn checked_table(image: &NativeImage) -> TableCheck {
    check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn metadata(image: &NativeImage, checked: &TableCheck, column: usize) -> Metadata {
    Metadata::capture(
        image,
        checked.schema(),
        checked.indexes(),
        column,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn record_rewrites_are_bound_to_the_captured_operation_roles_and_original_storage() {
    for code in [5, 6] {
        let original = source(code);
        let image = capture(&original.bytes);
        let checked = checked_table(&image);
        let column = checked
            .schema()
            .columns()
            .iter()
            .position(|field| field.name() == "Score")
            .unwrap();
        let protected = metadata(&image, &checked, column);
        let owner = Owner::OrdinaryCandidate { definition: 10 };
        let reference = (11 << 8) | 1;
        let (start, end) = model::row_range(&original.bytes, 11, 1);
        protected
            .require_record_rewrite(owner, reference, start..end, &mut budget(), &|| false)
            .unwrap();

        for (owner, reference, range) in [
            (Owner::GlobalFree, reference, start..end),
            (
                Owner::OrdinaryCandidate {
                    definition: model::OTHER as u32,
                },
                reference,
                start..end,
            ),
            (owner, 11 << 8, start..end),
            (owner, reference, start + 1..end),
        ] {
            assert_eq!(
                protected
                    .require_record_rewrite(owner, reference, range, &mut budget(), &|| false)
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::RowLayout))
            );
        }
    }
}

#[test]
fn complete_definition_chains_and_all_map_roles_are_forbidden_data_storage() {
    for code in [5, 6] {
        let mut original = source(code);
        let definition = model::OTHER * 4096;
        support::put32(&mut original.bytes, definition + 4, 593);
        support::put32(&mut original.bytes, definition + 8, 4097);
        let mut continuation = vec![0; 4096];
        continuation[..2].copy_from_slice(&[2, 1]);
        model::install(&mut original.bytes, 593, &continuation);

        let mut indirect = vec![0; 69];
        indirect[0] = 1;
        support::put32(&mut indirect, 1, 594);
        let map = support::data_page(
            0,
            &[
                (0, model::map(&[])),
                (0, model::map(&[])),
                (0, indirect),
                (0, model::map(&[])),
            ],
        );
        model::install(&mut original.bytes, model::OTHER_MAP, &map);
        let mut bitmap = vec![0; 4096];
        bitmap[..4].copy_from_slice(&[5, 1, 0, 0]);
        model::install(&mut original.bytes, 594, &bitmap);
        let tail = original.other_declarations;
        support::put16(&mut original.bytes, tail, 99);
        support::put32(&mut original.bytes, tail + 2, (581 << 8) | 2);
        support::put32(&mut original.bytes, tail + 6, (581 << 8) | 3);
        support::put16(&mut original.bytes, tail + 10, 0xffff);

        let image = capture(&original.bytes);
        let checked = checked_table(&image);
        // Id is not indexed. The two Score map containers are still protected
        // as unrelated metadata, together with historical column99 storage.
        let guard = metadata(&image, &checked, 0);
        for page in [1, 2, 7, 10, 11, 14, 20, 580, 581, 593, 594, 599] {
            assert_eq!(
                guard.require_data_page(page, &mut budget(), &|| false),
                Err(EditError::Unsupported(WriteBlocker::RowLayout))
            );
        }
        assert_eq!(
            guard.require_data_page(12, &mut budget(), &|| false),
            Ok(())
        );
        assert_eq!(
            guard.require_data_page(600, &mut budget(), &|| false),
            Ok(())
        );
    }
}

#[test]
fn physical_aliases_and_disjoint_rows_keep_authority_but_shared_roles_refuse() {
    for code in [5, 6] {
        let mut distinct = source(code);
        model::share_container_distinct_rows(&mut distinct);
        let image = capture(&distinct.bytes);
        let checked = checked_table(&image);
        assert_eq!(checked.indexes().len(), 4);
        let guard = metadata(&image, &checked, 1);
        assert_eq!(
            guard.require_data_page(14, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::RowLayout))
        );
        assert_eq!(
            guard.require_data_page(12, &mut budget(), &|| false),
            Ok(())
        );

        let mut shared = source(code);
        model::bind_historical_owned(&mut shared);
        let image = capture(&shared.bytes);
        let checked = checked_table(&image);
        assert_eq!(
            Metadata::capture(
                &image,
                checked.schema(),
                checked.indexes(),
                1,
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn incomplete_catalog_and_unbound_checked_indexes_do_not_grant_protection() {
    let mut original = source(6);
    original.bytes[original.other_declarations..(model::OTHER + 1) * 4096].fill(0);
    let image = capture(&original.bytes);
    let checked = checked_table(&image);
    assert_eq!(
        Metadata::capture(
            &image,
            checked.schema(),
            checked.indexes(),
            1,
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );

    let original = source(6);
    let image = capture(&original.bytes);
    let structural = checked_table(&image);
    assert_eq!(
        Metadata::capture(&image, structural.schema(), &[], 1, &mut budget(), &|| {
            false
        },)
        .err(),
        Some(EditError::Preservation)
    );
}

#[test]
fn resource_and_cancellation_failures_keep_their_types_and_allow_fresh_retry() {
    let original = source(6);
    let image = capture(&original.bytes);
    let checked = checked_table(&image);
    assert!(matches!(
        Metadata::capture(
            &image,
            checked.schema(),
            checked.indexes(),
            1,
            &mut ResourceBudget::new(0, 100_000, 100_000_000),
            &|| false,
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        Metadata::capture(
            &image,
            checked.schema(),
            checked.indexes(),
            1,
            &mut budget(),
            &|| true,
        ),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let guard = metadata(&image, &checked, 1);
    assert!(matches!(
        guard.require_data_page(600, &mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        guard.require_data_page(600, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(
        guard.require_data_page(600, &mut budget(), &|| false),
        Ok(())
    );
}
