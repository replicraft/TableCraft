//! Native filter membership and issued row identity, independent of UI text.
mod field;
use super::{request, stored::opened};
use crate::{
    CellKind, Command, Error, FilterComparison, FilterError, FilterHandle, FilterSelectionTarget,
    FilteredData, Response, SortSpec, SortTarget, Worker,
};
use tablecraft_expressions::EvalError;
use tablecraft_testkit::fixtures::access::{
    calculated_storage, canonical_fixed_creation, canonical_native17_creation,
    canonical_ordinary_wide_insert::{self, Case as OrdinaryInsertCase},
    canonical_variable_creation, complex_links, mutation_fixture,
};

fn selection(
    revision: u64,
    column: &str,
    ordinal: u32,
    base: Option<&FilterHandle>,
) -> FilterSelectionTarget {
    FilterSelectionTarget {
        document_revision: revision,
        table: "Items".into(),
        parent_ordinal: ordinal,
        column: column.into(),
        filter_id: base.map(|base| base.target().filter_id),
        enabled: base.is_some(),
    }
}

fn apply(
    worker: &Worker,
    target: FilterSelectionTarget,
    comparison: FilterComparison,
    base: Option<&FilterHandle>,
) -> Result<FilteredData, Error> {
    let Response::FilteredData(data) = request(
        worker,
        Command::FilterSelection {
            target,
            comparison,
            base: base.cloned(),
        },
    )?
    else {
        panic!("expected a complete native filtered page");
    };
    Ok(data)
}

fn page(
    worker: &Worker,
    handle: &FilterHandle,
    offset: u32,
    enabled: bool,
) -> Result<FilteredData, Error> {
    let Response::FilteredData(data) = request(
        worker,
        Command::FilterPage {
            handle: handle.clone(),
            offset,
            enabled,
        },
    )?
    else {
        panic!("expected an issued native filter page");
    };
    Ok(data)
}

#[test]
fn full_filter_map_crosses_pages_and_toggle_keeps_original_ordinals_and_match_count() {
    let rows: Vec<_> = (0..205).map(|index| (index + 1, index % 2)).collect();
    let original = mutation_fixture::with_rows(&rows);
    let (_directory, path, worker, revision) = opened(&original);
    let first = apply(
        &worker,
        selection(revision, "Score", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(first.source_rows, 205);
    assert_eq!(first.matched_rows, 103);
    assert_eq!(
        first.source_ordinals,
        (0..200).step_by(2).collect::<Vec<_>>()
    );
    assert_eq!(first.rows.len(), 100);
    assert!(first.has_more);
    assert_eq!(first.rows[99][0].text, "199");
    let second = page(&worker, &first.handle, 100, true).unwrap();
    assert_eq!(second.offset, 100);
    assert_eq!(second.source_ordinals, [200, 202, 204]);
    assert_eq!(second.rows[2][0].text, "205");
    assert!(!second.has_more);
    let unfiltered = page(&worker, &first.handle, 200, false).unwrap();
    assert_eq!(unfiltered.source_ordinals, [200, 201, 202, 203, 204]);
    assert_eq!(unfiltered.matched_rows, 103);
    assert_eq!(unfiltered.source_rows, 205);
    assert_eq!(unfiltered.target, first.target);
    assert_eq!(
        page(&worker, &first.handle, 100, true)
            .unwrap()
            .source_ordinals,
        [200, 202, 204]
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn same_field_replaces_its_criterion_while_other_fields_remain_eager_and_terms() {
    let original = mutation_fixture::with_rows(&[(1, 10), (2, 10), (3, 20), (4, 20)]);
    let (_directory, _path, worker, revision) = opened(&original);
    let first = apply(
        &worker,
        selection(revision, "Score", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(first.source_ordinals, [0, 1]);
    let second = apply(
        &worker,
        selection(revision, "Id", 0, Some(&first.handle)),
        FilterComparison::NotEqual,
        Some(&first.handle),
    )
    .unwrap();
    assert_eq!(second.source_ordinals, [1]);
    let replaced = apply(
        &worker,
        selection(revision, "Score", 1, Some(&second.handle)),
        FilterComparison::NotEqual,
        Some(&second.handle),
    )
    .unwrap();
    assert_eq!(replaced.source_ordinals, [2, 3]);
    // Published old capabilities remain valid and retain their original criteria.
    assert_eq!(
        page(&worker, &first.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [0, 1]
    );
    assert_eq!(
        page(&worker, &second.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [1]
    );
}

#[test]
fn null_empty_and_signed_zero_use_typed_predicates_instead_of_display_or_carrier_equality() {
    let original = canonical_native17_creation::source(
        canonical_fixed_creation::source(canonical_fixed_creation::Case::Nullable).bytes,
    );
    let (_directory, _path, worker, revision) = opened(&original);
    let nulls = apply(
        &worker,
        selection(revision, "Money", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(nulls.source_ordinals, [0]);
    let present = apply(
        &worker,
        selection(revision, "Money", 0, None),
        FilterComparison::NotEqual,
        None,
    )
    .unwrap();
    assert_eq!(present.source_ordinals, [1, 2]);
    let original = canonical_native17_creation::source(
        canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero).bytes,
    );
    let (_directory, _path, worker, revision) = opened(&original);
    assert_eq!(
        apply(
            &worker,
            selection(revision, "DoubleValue", 0, None),
            FilterComparison::Equal,
            None
        )
        .unwrap()
        .source_ordinals,
        [0, 1]
    );

    let source =
        canonical_variable_creation::source(canonical_variable_creation::Case::NullAndEmpty);
    let (_directory, _path, worker, revision) =
        opened(&canonical_native17_creation::source(source.bytes));
    let empty = apply(
        &worker,
        selection(revision, "TextValue", 1, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert!(!empty.source_ordinals.contains(&0));
    assert!(empty.source_ordinals.contains(&1));
}

#[test]
fn cross_worker_reused_revision_cannot_reuse_an_issued_filter_handle() {
    let original = mutation_fixture::database();
    let (_first_directory, _first_path, first_worker, first_revision) = opened(&original);
    let first = apply(
        &first_worker,
        selection(first_revision, "Score", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    let (_second_directory, _second_path, second_worker, second_revision) = opened(&original);
    assert_eq!(first_revision, second_revision);
    assert!(matches!(
        page(&second_worker, &first.handle, 0, true),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
    assert!(matches!(
        apply(
            &second_worker,
            selection(second_revision, "Score", 0, Some(&first.handle)),
            FilterComparison::Equal,
            Some(&first.handle)
        ),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
}

#[test]
fn canceled_and_failed_candidates_preserve_the_old_capability_and_view() {
    let original = mutation_fixture::database();
    let (directory, path, _worker, revision) = opened(&original);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    let Response::FilteredData(first) = document
        .filter_selection(
            &selection(revision, "Score", 0, None),
            FilterComparison::Equal,
            None,
            &|| false,
        )
        .unwrap()
    else {
        panic!("expected initial filter");
    };
    assert!(matches!(
        document.filter_selection(
            &selection(revision, "Id", 0, Some(&first.handle)),
            FilterComparison::Equal,
            Some(&first.handle),
            &|| true
        ),
        Err(Error::Cancelled)
    ));
    assert!(
        document
            .filter_selection(
                &selection(revision, "missing", 0, None),
                FilterComparison::Equal,
                None,
                &|| false
            )
            .is_err()
    );
    let Response::FilteredData(restored) = document
        .filter_page(&first.handle, 0, true, &|| false)
        .unwrap()
    else {
        panic!("old handle must remain valid");
    };
    assert_eq!(restored.source_ordinals, [0]);
    assert_eq!(std::fs::read(path).unwrap(), original);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn malformed_eof_never_publishes_a_filter_prefix() {
    let mut original = mutation_fixture::database();
    const PAGE_BYTES: usize = 4096;
    const USER_DEFINITION: usize = 10;
    const DECLARED_ROWS: usize = 16;
    let position = USER_DEFINITION * PAGE_BYTES + DECLARED_ROWS;
    original[position..position + 4].copy_from_slice(&4_u32.to_le_bytes());
    let (_directory, _path, worker, revision) = opened(&original);
    assert!(
        apply(
            &worker,
            selection(revision, "Score", 0, None),
            FilterComparison::Equal,
            None
        )
        .is_err()
    );
}

#[test]
fn explicit_definition_reapply_rescans_new_source_without_reusing_old_row_rights() {
    let original = mutation_fixture::with_rows(&[(1, 10), (2, 20), (3, 10)]);
    let (_directory, path, worker, revision) = opened(&original);
    let first = apply(
        &worker,
        selection(revision, "Score", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(first.source_ordinals, [0, 2]);
    let definition = first.handle.definition();
    // The old selected row now has a different value; preferences retain typed10.
    std::fs::write(
        &path,
        mutation_fixture::with_rows(&[(1, 20), (2, 10), (3, 10)]),
    )
    .unwrap();
    let Response::Opened {
        document_revision: next_revision,
        ..
    } = request(&worker, super::open(&path, "")).unwrap()
    else {
        panic!("reopened document");
    };
    assert!(matches!(
        page(&worker, &first.handle, 0, true),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
    let Response::FilteredData(rebound) = request(
        &worker,
        Command::ReapplyFilter {
            definition: definition.clone(),
            document_revision: next_revision,
            enabled: true,
            offset: 100,
        },
    )
    .unwrap() else {
        panic!("fresh explicit filter reapplication");
    };
    assert_ne!(rebound.target.filter_id, first.target.filter_id);
    assert_eq!(rebound.target.document_revision, next_revision);
    assert_eq!(rebound.matched_rows, 2);
    assert_eq!(rebound.offset, 1);
    assert_eq!(rebound.source_ordinals, [2]);
    assert_eq!(
        page(&worker, &rebound.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [1, 2]
    );
    let Response::FilteredData(disabled) = request(
        &worker,
        Command::ReapplyFilter {
            definition,
            document_revision: next_revision,
            enabled: false,
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("disabled filter preferences remain disabled");
    };
    assert!(!disabled.enabled);
    assert_eq!(disabled.source_ordinals, [0, 1, 2]);
    assert_eq!(disabled.matched_rows, 2);
}

#[test]
fn a_false_earlier_filter_does_not_hide_a_later_nonfinite_comparison_error() {
    let source = canonical_ordinary_wide_insert::source(OrdinaryInsertCase::FixedSpecial);
    let (_directory, _path, worker, revision) = opened(&source.after);
    let first = apply(
        &worker,
        selection(revision, "Int32Value", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(first.source_ordinals, [0, 1]);
    // The third row fails Int32Value=0, but its NaN must still be compared
    // against the second typed criterion instead of being short-circuited away.
    assert!(matches!(
        apply(
            &worker,
            selection(revision, "DoubleValue", 0, Some(&first.handle)),
            FilterComparison::Equal,
            Some(&first.handle),
        ),
        Err(Error::Conversion(EvalError::NonFinite))
    ));
    assert_eq!(
        page(&worker, &first.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [0, 1]
    );
}

#[test]
fn sparse_mixed_pages_keep_original_collection_parents_and_cached_null_provenance() {
    let original = complex_links::presence_cases();
    let (_directory, path, worker, revision) = opened(&original);
    let filtered = apply(
        &worker,
        selection(revision, "Id", 1, None),
        FilterComparison::NotEqual,
        None,
    )
    .unwrap();
    assert_eq!(filtered.source_ordinals, [0, 2, 3]);
    assert!(matches!(
        apply(
            &worker,
            selection(revision, &filtered.columns[1].name, 0, None),
            FilterComparison::Equal,
            None,
        ),
        Err(Error::Filter(FilterError::UnsupportedField))
    ));
    assert!(matches!(
        sort(
            &worker,
            revision,
            Some(&filtered.handle),
            true,
            Some(SortSpec {
                column: filtered.columns[1].name.clone(),
                descending: false,
            }),
        ),
        Err(Error::Filter(FilterError::UnsupportedField))
    ));
    assert_eq!(
        filtered.rows[0][1].kind,
        CellKind::Multivalue {
            assigned: true,
            members: 3
        }
    );
    assert_eq!(
        filtered.rows[1][1].kind,
        CellKind::Multivalue {
            assigned: true,
            members: 0
        }
    );
    assert_eq!(
        filtered.rows[2][1].kind,
        CellKind::Multivalue {
            assigned: false,
            members: 0
        }
    );
    let tail = page(&worker, &filtered.handle, 1, true).unwrap();
    assert_eq!(tail.offset, 1);
    assert_eq!(tail.source_ordinals, [2, 3]);
    assert_eq!(tail.rows[0][0].text, "3");
    assert_eq!(
        tail.rows[0][1].kind,
        CellKind::Multivalue {
            assigned: true,
            members: 0
        }
    );
    assert_eq!(
        tail.rows[1][1].kind,
        CellKind::Multivalue {
            assigned: false,
            members: 0
        }
    );
    assert_eq!(std::fs::read(path).unwrap(), original);

    let original = calculated_storage::database_with_expression(&[None, Some(88), None], "1/0");
    let (_directory, path, worker, revision) = opened(&original);
    let filtered = apply(
        &worker,
        selection(revision, "Id", 1, None),
        FilterComparison::NotEqual,
        None,
    )
    .unwrap();
    assert_eq!(filtered.source_ordinals, [0, 2]);
    for row in &filtered.rows {
        assert_eq!(row[1].text, "NULL");
        assert!(matches!(
            row[1].kind,
            CellKind::Scalar {
                is_null: true,
                stored_calculation: true,
                ..
            }
        ));
    }
    let tail = page(&worker, &filtered.handle, 1, true).unwrap();
    assert_eq!(tail.source_ordinals, [2]);
    assert!(matches!(
        tail.rows[0][1].kind,
        CellKind::Scalar {
            is_null: true,
            stored_calculation: true,
            ..
        }
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

fn sort(
    worker: &Worker,
    revision: u64,
    base: Option<&FilterHandle>,
    enabled: bool,
    spec: Option<SortSpec>,
) -> Result<FilteredData, Error> {
    let Response::FilteredData(data) = request(
        worker,
        Command::SortView {
            target: SortTarget {
                document_revision: revision,
                table: "Items".into(),
                filter_id: base.map(|handle| handle.target().filter_id),
            },
            sort: spec,
            base: base.cloned(),
            enabled,
        },
    )?
    else {
        panic!("expected a complete native sorted page");
    };
    Ok(data)
}

#[test]
fn native_sort_retains_filter_membership_and_source_positions_across_modes_and_clear() {
    let bytes = mutation_fixture::with_rows(&[(1, 30), (2, 10), (3, 20), (4, 10)]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let ascending = SortSpec {
        column: "Score".into(),
        descending: false,
    };
    let sorted = sort(&worker, revision, None, false, Some(ascending.clone())).unwrap();
    assert_eq!(sorted.source_ordinals, [1, 3, 2, 0]);
    assert_eq!(sorted.rows[0][0].text, "2");
    assert!(!sorted.enabled);
    assert_eq!(sorted.filter_count, 0);
    assert_eq!(sorted.handle.display_offset_for_ordinal(false, 0), Some(3));
    let mut selected = selection(revision, "Score", 1, Some(&sorted.handle));
    selected.enabled = false;
    let filtered = apply(
        &worker,
        selected,
        FilterComparison::Equal,
        Some(&sorted.handle),
    )
    .unwrap();
    assert_eq!(filtered.source_ordinals, [1, 3]);
    assert_eq!(filtered.sort, Some(ascending.clone()));
    assert_eq!(filtered.filter_count, 1);
    assert_eq!(filtered.handle.display_offset_for_ordinal(false, 0), None);
    let off = page(&worker, &filtered.handle, 0, false).unwrap();
    assert_eq!(off.target, filtered.target);
    assert_eq!(off.source_ordinals, [1, 3, 2, 0]);
    assert_eq!(off.handle.display_offset_for_ordinal(true, 1), None);
    assert_eq!(
        page(&worker, &filtered.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [1, 3]
    );
    let mut fresh = selection(revision, "Id", 0, Some(&off.handle));
    fresh.enabled = false;
    let replacement = apply(&worker, fresh, FilterComparison::Equal, Some(&off.handle)).unwrap();
    assert_eq!(replacement.source_ordinals, [0]);
    assert_eq!(replacement.filter_count, 1);
    assert_eq!(replacement.sort, Some(ascending));
    let Response::FilteredData(cleared) = request(
        &worker,
        Command::ClearFilter {
            handle: filtered.handle.clone(),
        },
    )
    .unwrap() else {
        panic!("native clear retains sorting");
    };
    assert_eq!(cleared.source_ordinals, [1, 3, 2, 0]);
    assert_eq!(cleared.filter_count, 0);
    assert!(!cleared.enabled);
    let removed = sort(&worker, revision, Some(&filtered.handle), true, None).unwrap();
    assert_eq!(removed.source_ordinals, [1, 3]);
    assert!(removed.sort.is_none());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn sorting_excluded_nonfinite_keys_does_not_preflight_them_for_a_future_toggle() {
    let source = canonical_ordinary_wide_insert::source(OrdinaryInsertCase::FixedSpecial);
    let (_directory, _path, worker, revision) = opened(&source.after);
    let filtered = apply(
        &worker,
        selection(revision, "Int32Value", 0, None),
        FilterComparison::Equal,
        None,
    )
    .unwrap();
    assert_eq!(filtered.source_ordinals, [0, 1]);
    let sorted = sort(
        &worker,
        revision,
        Some(&filtered.handle),
        true,
        Some(SortSpec {
            column: "DoubleValue".into(),
            descending: false,
        }),
    )
    .unwrap();
    assert_eq!(sorted.source_ordinals, [0, 1]);
    // The hidden third record contains NaN. Only explicitly requesting the
    // unfiltered order evaluates that key; a failure cannot replace this view.
    assert!(page(&worker, &sorted.handle, 0, false).is_err());
    assert_eq!(
        page(&worker, &sorted.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [0, 1]
    );
    assert_eq!(sorted.handle.display_offset_for_ordinal(false, 2), None);
}

#[test]
fn sorted_definition_rebinds_after_reopen_and_never_grants_old_source_rights() {
    let bytes = mutation_fixture::with_rows(&[(1, 30), (2, 10), (3, 20)]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let sorted = sort(
        &worker,
        revision,
        None,
        false,
        Some(SortSpec {
            column: "Score".into(),
            descending: true,
        }),
    )
    .unwrap();
    let definition = sorted.handle.definition();
    let Response::Opened {
        document_revision: fresh_revision,
        ..
    } = request(
        &worker,
        Command::Open {
            path,
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("fresh source");
    };
    assert!(matches!(
        page(&worker, &sorted.handle, 0, false),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
    let Response::FilteredData(rebound) = request(
        &worker,
        Command::ReapplyFilter {
            definition,
            document_revision: fresh_revision,
            enabled: false,
            offset: 1,
        },
    )
    .unwrap() else {
        panic!("fresh native sort preferences");
    };
    assert_eq!(rebound.source_ordinals, [2, 1]);
    assert_eq!(rebound.offset, 1);
    assert_ne!(rebound.target.filter_id, sorted.target.filter_id);
    assert_eq!(rebound.filter_count, 0);
}
