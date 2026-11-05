//! Full immutable native-table Find, independent of loaded browser pages.
use super::{request, stored::opened};
use crate::{
    CellKind, Command, Error, FilterComparison, FilterError, FilterHandle, FilterSelectionTarget,
    FilteredData, FindDirection, FindError, FindMatch, FindPosition, FindRequest, FindScope,
    FindSpan, FindTarget, Response, SortSpec, SortTarget, Worker,
};
use tablecraft_display::Options;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation, canonical_native17_creation, complex_links, mutation_fixture,
};

pub(super) fn search(revision: u64, pattern: &str) -> FindRequest {
    FindRequest {
        target: FindTarget {
            document_revision: revision,
            table: "Items".into(),
        },
        scope: FindScope::Field("Id".into()),
        pattern: pattern.into(),
        matching: FindMatch::Whole,
        direction: FindDirection::All,
        start: None,
        match_case: false,
        formatted: false,
        format_options: Options {
            system_locale: Some("en-US".into()),
            ..Options::default()
        },
    }
}

pub(super) fn find(worker: &Worker, expected: FindRequest) -> Result<Option<FindPosition>, Error> {
    let Response::Found {
        request: actual,
        position,
    } = request(
        worker,
        Command::FindNext {
            request: expected.clone(),
        },
    )?
    else {
        panic!("expected a native Find response");
    };
    assert_eq!(actual, expected);
    Ok(position)
}

#[test]
fn native_find_crosses_browser_pages_and_keeps_exact_source_field_and_revision() {
    let rows: Vec<_> = (1..=105).map(|number| (number, number * 10)).collect();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let mut options = search(revision, "104");
    let hit = find(&worker, options.clone()).unwrap().unwrap();
    assert_eq!(
        hit,
        FindPosition {
            parent_ordinal: 103,
            column: "Id".into(),
            span: Some(FindSpan {
                start_utf16: 0,
                end_utf16: 3
            })
        }
    );
    options.scope = FindScope::Table;
    options.pattern = "1050".into();
    let hit = find(&worker, options.clone()).unwrap().unwrap();
    assert_eq!(hit.parent_ordinal, 104);
    assert_eq!(hit.column, "Score");
    options.pattern = "absent".into();
    assert!(find(&worker, options.clone()).unwrap().is_none());
    options.target.document_revision += 1;
    assert!(matches!(find(&worker, options), Err(Error::StaleDocument)));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn repeated_find_consumes_same_cell_spans_then_wraps_and_up_returns_previous_occurrence() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1111, 10)], [9, 4, 0, 0]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let mut options = search(revision, "11");
    options.matching = FindMatch::Any;
    let first = find(&worker, options.clone()).unwrap().unwrap();
    assert_eq!(
        first.span,
        Some(FindSpan {
            start_utf16: 0,
            end_utf16: 2
        })
    );
    options.start = Some(first.clone());
    let second = find(&worker, options.clone()).unwrap().unwrap();
    assert_eq!(
        second.span,
        Some(FindSpan {
            start_utf16: 2,
            end_utf16: 4
        })
    );
    options.start = Some(second.clone());
    assert_eq!(find(&worker, options.clone()).unwrap(), Some(first.clone()));
    options.direction = FindDirection::Up;
    assert_eq!(find(&worker, options.clone()).unwrap(), Some(first));
    options.direction = FindDirection::Down;
    assert!(find(&worker, options).unwrap().is_none());
}

#[test]
fn find_never_reports_not_found_or_a_prefix_hit_before_checked_eof_and_valid_anchor() {
    let rows: Vec<_> = (1..=105).map(|number| (number, number)).collect();
    let mut bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    // Independent original fixture table definition: claim one nonexistent row.
    const USER_TABLE_DEFINITION_PAGE: usize = 10;
    const PAGE_BYTES: usize = 4096;
    const DECLARED_ROWS_OFFSET: usize = 16;
    let offset = USER_TABLE_DEFINITION_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET;
    bytes[offset..offset + 4].copy_from_slice(&106_u32.to_le_bytes());
    let (_directory, _path, worker, revision) = opened(&bytes);
    assert!(find(&worker, search(revision, "absent")).is_err());
    assert!(find(&worker, search(revision, "1")).is_err());

    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 10)], [9, 4, 0, 0]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let mut options = search(revision, "1");
    options.start = Some(FindPosition {
        parent_ordinal: 99,
        column: "Id".into(),
        span: None,
    });
    assert!(matches!(
        find(&worker, options),
        Err(Error::Find(FindError::InvalidTarget))
    ));
}

#[test]
fn unsupported_unformatted_date_and_unknown_collation_do_not_become_not_found() {
    let source = canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
    let (_directory, _path, worker, revision) =
        opened(&canonical_native17_creation::source(source.bytes));
    let mut options = search(revision, "2000");
    options.scope = FindScope::Field("DateValue".into());
    assert!(matches!(
        find(&worker, options),
        Err(Error::Find(FindError::UnformattedDate))
    ));

    let bytes = mutation_fixture::with_rows(&[(1, 10)]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    assert!(find(&worker, search(revision, "1")).is_err());
}

#[test]
fn native_null_criteria_are_typed_and_formatted_currency_uses_captured_display_policy() {
    let source = canonical_fixed_creation::source(canonical_fixed_creation::Case::Nullable);
    let (_directory, _path, worker, revision) =
        opened(&canonical_native17_creation::source(source.bytes));
    let mut options = search(revision, "Is Null");
    options.scope = FindScope::Field("Money".into());
    assert!(find(&worker, options.clone()).unwrap().is_some());
    options.pattern = "NULL*".into();
    options.matching = FindMatch::Any;
    assert!(matches!(
        find(&worker, options),
        Err(Error::Find(FindError::UnformattedNumber))
    ));

    let source = canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
    let (_directory, _path, worker, revision) =
        opened(&canonical_native17_creation::source(source.bytes));
    let mut options = search(revision, "€0.00");
    options.scope = FindScope::Field("Money".into());
    options.formatted = true;
    options.format_options.currency = tablecraft_display::CurrencyChoice::Eur;
    assert!(find(&worker, options.clone()).unwrap().is_some());
    options.pattern = "$0.00".into();
    assert!(find(&worker, options).unwrap().is_none());
}

#[test]
fn find_cancellation_and_pattern_ceiling_return_errors_instead_of_negative_publication() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 10)], [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    assert!(matches!(
        find(&worker, search(revision, "")),
        Err(Error::Find(FindError::EmptyPattern))
    ));
    let mut options = search(revision, "1");
    options.pattern = "x".repeat(65_537);
    assert!(matches!(
        find(&worker, options),
        Err(Error::Find(FindError::PatternLimit))
    ));

    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.find_next(&search(revision, "1"), &|| true),
        Err(Error::Cancelled)
    ));
}

#[test]
fn filtered_find_excludes_hidden_matches_crosses_pages_and_binds_issued_source() {
    let rows: Vec<_> = (0..205).map(|index| (index + 1, index % 2)).collect();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let Response::FilteredData(filtered) = request(
        &worker,
        Command::FilterSelection {
            target: FilterSelectionTarget {
                document_revision: revision,
                table: "Items".into(),
                parent_ordinal: 0,
                column: "Score".into(),
                filter_id: None,
                enabled: false,
            },
            comparison: FilterComparison::Equal,
            base: None,
        },
    )
    .unwrap() else {
        panic!("expected complete filter membership");
    };
    let search_filtered = |options: FindRequest, enabled| {
        request(
            &worker,
            Command::FindFiltered {
                handle: filtered.handle.clone(),
                request: options,
                enabled,
            },
        )
    };

    let Response::Found { position, .. } = search_filtered(search(revision, "204"), true).unwrap()
    else {
        panic!("expected complete filtered Find");
    };
    assert!(
        position.is_none(),
        "a hidden match must not escape the retained map"
    );
    let Response::Found { position, .. } = search_filtered(search(revision, "205"), true).unwrap()
    else {
        panic!("expected complete filtered Find");
    };
    let found = position.unwrap();
    assert_eq!(found.parent_ordinal, 204);
    assert_eq!(
        filtered.handle.display_offset_for_ordinal(true, 204),
        Some(102)
    );
    assert_eq!(filtered.handle.display_offset_for_ordinal(true, 203), None);

    let mut invalid_anchor = search(revision, "205");
    invalid_anchor.start = Some(FindPosition {
        parent_ordinal: 203,
        column: "Id".into(),
        span: None,
    });
    assert!(matches!(
        search_filtered(invalid_anchor, true),
        Err(Error::Find(FindError::InvalidTarget))
    ));
    let Response::Found { position, .. } = search_filtered(search(revision, "204"), false).unwrap()
    else {
        panic!("expected Find with the retained filter toggled off");
    };
    assert_eq!(position.unwrap().parent_ordinal, 203);
    assert_eq!(
        filtered.handle.display_offset_for_ordinal(false, 203),
        Some(203)
    );

    let (_other_directory, _other_path, other_worker, other_revision) = opened(&bytes);
    assert_eq!(revision, other_revision);
    assert!(matches!(
        request(
            &other_worker,
            Command::FindFiltered {
                handle: filtered.handle,
                request: search(other_revision, "205"),
                enabled: true,
            }
        ),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn complex_fields_refuse_find_without_exposing_internal_owner_keys() {
    let original = complex_links::presence_cases();
    let (_directory, path, worker, revision) = opened(&original);
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected mixed native table");
    };
    assert!(matches!(rows[0][1].kind, CellKind::Multivalue { .. }));
    let mut options = search(revision, "10");
    options.scope = FindScope::Field(columns[1].name.clone());
    assert!(matches!(
        find(&worker, options.clone()),
        Err(Error::Find(FindError::UnsupportedField))
    ));
    options.scope = FindScope::Table;
    assert!(matches!(
        find(&worker, options),
        Err(Error::Find(FindError::UnsupportedField))
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn sorted_find_uses_display_order_for_occurrences_directions_wrap_and_filtered_pages() {
    let rows: Vec<_> = (0..105)
        .map(|ordinal| {
            let score = match ordinal {
                0 => 20,
                1 => 30,
                104 => 10,
                _ => 40 + ordinal,
            };
            (1111, score)
        })
        .collect();
    let bytes = mutation_fixture::with_rows_and_sort_order(&rows, [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let sorted = |base: Option<FilterHandle>, enabled| {
        let Response::FilteredData(data) = request(
            &worker,
            Command::SortView {
                target: SortTarget {
                    document_revision: revision,
                    table: "Items".into(),
                    filter_id: base.as_ref().map(|handle| handle.target().filter_id),
                },
                sort: Some(SortSpec {
                    column: "Score".into(),
                    descending: false,
                }),
                base,
                enabled,
            },
        )
        .unwrap() else {
            panic!("expected complete native sorted membership");
        };
        data
    };
    let view = sorted(None, false);
    assert_eq!(&view.source_ordinals[..3], [104, 0, 1]);
    assert!(view.has_more);
    let search_view = |view: &FilteredData, options: FindRequest| {
        let Response::Found { position, .. } = request(
            &worker,
            Command::FindFiltered {
                handle: view.handle.clone(),
                request: options,
                enabled: view.enabled,
            },
        )
        .unwrap() else {
            panic!("expected complete Find over sorted membership");
        };
        position
    };
    let mut options = search(revision, "11");
    options.matching = FindMatch::Any;
    let first = search_view(&view, options.clone()).unwrap();
    assert_eq!(first.parent_ordinal, 104);
    assert_eq!(first.span.as_ref().unwrap().start_utf16, 0);
    options.start = Some(first);
    let second = search_view(&view, options.clone()).unwrap();
    assert_eq!(second.parent_ordinal, 104);
    assert_eq!(second.span.as_ref().unwrap().start_utf16, 2);
    options.start = Some(second.clone());
    options.direction = FindDirection::Down;
    let next = search_view(&view, options.clone()).unwrap();
    assert_eq!(next.parent_ordinal, 0);
    assert_eq!(next.span.as_ref().unwrap().start_utf16, 0);
    options.start = Some(next);
    options.direction = FindDirection::Up;
    let previous = search_view(&view, options.clone()).unwrap();
    assert_eq!(previous, second);

    let mut tail = search(revision, "143");
    tail.scope = FindScope::Field("Score".into());
    let last = search_view(&view, tail.clone()).unwrap();
    assert_eq!(last.parent_ordinal, 103);
    assert_eq!(
        view.handle.display_offset_for_ordinal(view.enabled, 103),
        Some(104)
    );
    options.start = Some(FindPosition {
        parent_ordinal: 103,
        column: "Id".into(),
        span: Some(FindSpan {
            start_utf16: 2,
            end_utf16: 4,
        }),
    });
    options.direction = FindDirection::All;
    let wrapped = search_view(&view, options.clone()).unwrap();
    assert_eq!(wrapped.parent_ordinal, 104);
    assert_eq!(wrapped.span.as_ref().unwrap().start_utf16, 0);
    options.direction = FindDirection::Down;
    assert!(search_view(&view, options).is_none());

    let Response::FilteredData(filtered) = request(
        &worker,
        Command::FilterSelection {
            target: FilterSelectionTarget {
                document_revision: revision,
                table: "Items".into(),
                parent_ordinal: 0,
                column: "Score".into(),
                filter_id: None,
                enabled: false,
            },
            comparison: FilterComparison::NotEqual,
            base: None,
        },
    )
    .unwrap() else {
        panic!("expected complete selection membership");
    };
    let view = sorted(Some(filtered.handle), true);
    assert_eq!(&view.source_ordinals[..2], [104, 1]);
    let mut after_first = search(revision, "11");
    after_first.matching = FindMatch::Any;
    after_first.start = Some(second);
    assert_eq!(search_view(&view, after_first).unwrap().parent_ordinal, 1);
    assert_eq!(search_view(&view, tail).unwrap().parent_ordinal, 103);
    assert_eq!(
        view.handle.display_offset_for_ordinal(view.enabled, 103),
        Some(103)
    );
    let Response::FilteredData(empty) = request(
        &worker,
        Command::FilterSelection {
            target: FilterSelectionTarget {
                document_revision: revision,
                table: "Items".into(),
                parent_ordinal: 0,
                column: "Id".into(),
                filter_id: None,
                enabled: false,
            },
            comparison: FilterComparison::NotEqual,
            base: None,
        },
    )
    .unwrap() else {
        panic!("expected complete empty membership");
    };
    let off = sorted(Some(empty.handle), false);
    assert_eq!(off.matched_rows, 0);
    assert_eq!(off.source_rows, 105);
    assert!(
        matches!(
            request(
                &worker,
                Command::FindFiltered {
                    handle: off.handle,
                    request: search(revision, "absent"),
                    enabled: true,
                }
            ),
            Err(Error::Find(FindError::InvalidTarget))
        ),
        "an uncached empty mode must refuse before returning Not found"
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
