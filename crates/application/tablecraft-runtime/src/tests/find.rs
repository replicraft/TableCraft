//! Full immutable native-table Find, independent of loaded browser pages.
use super::{request, stored::opened};
use crate::{
    CellKind, Command, Error, FilterComparison, FilterError, FilterSelectionTarget, FindDirection,
    FindError, FindMatch, FindPosition, FindRequest, FindScope, FindSpan, FindTarget, Response,
    Worker,
};
use tablecraft_display::Options;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation, canonical_native17_creation, complex_links, mutation_fixture,
};

fn search(revision: u64, pattern: &str) -> FindRequest {
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

fn find(worker: &Worker, expected: FindRequest) -> Result<Option<FindPosition>, Error> {
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
