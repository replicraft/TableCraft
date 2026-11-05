//! Complete native view totals reuse query arithmetic and preserve source rights.
use super::{request, stored::opened};
use crate::{
    Command, Error, FieldFilterOperator, FieldFilterTarget, FilterHandle, Response,
    TotalAggregate as Aggregate, TotalSpec, TotalsData, TotalsTarget, Worker,
};
use tablecraft_display::{CurrencyChoice, Engine, Options, ScalarPresentation};
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as fixed,
    canonical_ordinary_wide_insert::{self, Case as InsertCase},
    canonical_variable_creation as variable, field_presentation, mutation_fixture,
};

fn target(revision: u64, base: Option<&FilterHandle>, enabled: bool) -> TotalsTarget {
    TotalsTarget {
        document_revision: revision,
        table: "Items".into(),
        filter_id: base.map(|handle| handle.target().filter_id),
        enabled,
    }
}

fn total(
    worker: &Worker,
    target: TotalsTarget,
    specs: &[(&str, Aggregate)],
    base: Option<&FilterHandle>,
) -> Result<TotalsData, Error> {
    let Response::Totals(data) = request(
        worker,
        Command::Totals {
            target,
            specs: specs
                .iter()
                .map(|(column, aggregate)| TotalSpec {
                    column: (*column).into(),
                    aggregate: *aggregate,
                })
                .collect(),
            base: base.cloned(),
        },
    )?
    else {
        panic!("expected a complete derived totals publication");
    };
    Ok(data)
}

#[test]
fn totals_cover_all_105_native_rows_and_all_numeric_kernels() {
    let records: Vec<_> = (0..105).map(|index| (index + 1, index)).collect();
    let bytes = mutation_fixture::with_rows(&records);
    let (_directory, path, worker, revision) = opened(&bytes);
    for (aggregate, expected) in [
        (Aggregate::Count, 105.0),
        (Aggregate::Sum, 5460.0),
        (Aggregate::Average, 52.0),
        (Aggregate::Minimum, 0.0),
        (Aggregate::Maximum, 104.0),
        (Aggregate::Variance, 927.5),
        (Aggregate::StandardDeviation, 927.5_f64.sqrt()),
    ] {
        let data = total(
            &worker,
            target(revision, None, false),
            &[("Score", aggregate)],
            None,
        )
        .unwrap();
        assert_eq!((data.source_rows, data.view_rows), (105, 105));
        let actual: f64 = data.cells[0].text.parse().unwrap();
        assert!((actual - expected).abs() < 1e-10, "{aggregate:?}: {actual}");
        assert_eq!(data.specs[0].column, "Score");
    }
    let together = total(
        &worker,
        target(revision, None, false),
        &[("Score", Aggregate::Sum), ("Id", Aggregate::Count)],
        None,
    )
    .unwrap();
    assert_eq!(
        together
            .cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["5460", "105"]
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn active_and_off_filter_totals_keep_exact_membership_and_old_handle() {
    let bytes =
        mutation_fixture::with_rows(&(0..105).map(|index| (index + 1, index)).collect::<Vec<_>>());
    let (_directory, _path, worker, revision) = opened(&bytes);
    let Response::FilteredData(filtered) = request(
        &worker,
        Command::ApplyFieldFilter {
            target: FieldFilterTarget {
                document_revision: revision,
                table: "Items".into(),
                column: "Score".into(),
                filter_id: None,
                enabled: false,
            },
            operator: FieldFilterOperator::GreaterOrEqual,
            inputs: vec!["100".into()],
            base: None,
        },
    )
    .unwrap() else {
        panic!("checked native filter");
    };
    let active = total(
        &worker,
        target(revision, Some(&filtered.handle), true),
        &[("Score", Aggregate::Sum)],
        Some(&filtered.handle),
    )
    .unwrap();
    assert_eq!((active.source_rows, active.view_rows), (105, 5));
    assert_eq!(active.cells[0].text, "510");
    let off = total(
        &worker,
        target(revision, Some(&filtered.handle), false),
        &[("Score", Aggregate::Sum)],
        Some(&filtered.handle),
    )
    .unwrap();
    assert_eq!((off.source_rows, off.view_rows), (105, 105));
    assert_eq!(off.cells[0].text, "5460");
    let empty = total(
        &worker,
        target(revision, Some(&filtered.handle), true),
        &[],
        Some(&filtered.handle),
    )
    .unwrap();
    assert_eq!((empty.source_rows, empty.view_rows), (105, 5));
    assert!(empty.cells.is_empty());
    let (_other_directory, _other_path, other, other_revision) = opened(&bytes);
    assert_eq!(other_revision, revision);
    assert!(
        total(
            &other,
            target(revision, Some(&filtered.handle), true),
            &[("Score", Aggregate::Sum)],
            Some(&filtered.handle)
        )
        .is_err()
    );
    let Response::FilteredData(page) = request(
        &worker,
        Command::FilterPage {
            handle: filtered.handle,
            offset: 0,
            enabled: true,
        },
    )
    .unwrap() else {
        panic!("old view retained");
    };
    assert_eq!(page.source_ordinals, [100, 101, 102, 103, 104]);
}

#[test]
fn metadata_is_row_independent_and_count_ignores_null_but_counts_empty_text() {
    let source = variable::source(variable::Case::Empty);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    let Response::TotalsDescription(description) = request(
        &worker,
        Command::DescribeTotals {
            target: target(revision, None, false),
        },
    )
    .unwrap() else {
        panic!("metadata without records");
    };
    let text = description
        .fields
        .iter()
        .find(|field| field.column.name == "TextValue")
        .unwrap();
    assert_eq!(text.aggregates, [Aggregate::Count]);
    assert_eq!(
        total(
            &worker,
            target(revision, None, false),
            &[("TextValue", Aggregate::Count)],
            None
        )
        .unwrap()
        .cells[0]
            .text,
        "0"
    );
    assert_eq!(
        total(
            &worker,
            target(revision, None, false),
            &[("Id", Aggregate::Sum)],
            None
        )
        .unwrap()
        .cells[0]
            .text,
        "NULL"
    );
    let source = variable::source(variable::Case::NullAndEmpty);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    assert_eq!(
        total(
            &worker,
            target(revision, None, false),
            &[("TextValue", Aggregate::Count)],
            None
        )
        .unwrap()
        .cells[0]
            .text,
        "2"
    );
    assert!(
        total(
            &worker,
            target(revision, None, false),
            &[("TextValue", Aggregate::Minimum)],
            None
        )
        .is_err()
    );
    let bytes = mutation_fixture::with_rows(&[(1, 2)]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    for aggregate in [Aggregate::Variance, Aggregate::StandardDeviation] {
        assert_eq!(
            total(
                &worker,
                target(revision, None, false),
                &[("Score", aggregate)],
                None
            )
            .unwrap()
            .cells[0]
                .text,
            "NULL"
        );
    }
}

#[test]
fn derived_date_and_currency_mean_keep_display_lineage_without_storage_rounding() {
    let bytes = field_presentation::score64(&[], 8, &[0.0_f64.to_bits(), 2.0_f64.to_bits()]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let data = total(
        &worker,
        target(revision, None, false),
        &[("Score", Aggregate::Average)],
        None,
    )
    .unwrap();
    let Some(ScalarPresentation::DateTime { calendar, .. }) = data.cells[0].presentation else {
        panic!("explicit derived legacy date");
    };
    assert_eq!(calendar, jiff::civil::date(1899, 12, 31).at(0, 0, 0, 0));
    let bytes = field_presentation::score64(&[], 5, &[12344, 12345]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let data = total(
        &worker,
        target(revision, None, false),
        &[("Score", Aggregate::Average)],
        None,
    )
    .unwrap();
    let Some(presentation @ ScalarPresentation::CurrencyAverage { bits, .. }) =
        data.cells[0].presentation
    else {
        panic!("query Double mean retained");
    };
    assert_eq!(f64::from_bits(bits), 1.23445);
    assert_eq!(data.cells[0].text, "1.23445");
    let mut policy = Options {
        system_locale: Some("en-US".into()),
        currency: CurrencyChoice::Usd,
        ..Options::default()
    };
    assert_eq!(Engine::new(&policy).unwrap().format(presentation), "$1.23");
    policy.currency = CurrencyChoice::Raw;
    assert_eq!(
        Engine::new(&policy).unwrap().format(presentation),
        "1.23445"
    );
}

#[test]
fn null_numeric_inputs_and_singleton_statistics_reuse_query_semantics() {
    let source = fixed::source(fixed::Case::Nullable);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    for (aggregate, expected) in [
        (Aggregate::Count, "2"),
        (Aggregate::Sum, "200"),
        (Aggregate::Average, "100"),
        (Aggregate::Minimum, "0"),
        (Aggregate::Maximum, "200"),
        (Aggregate::Variance, "20000"),
    ] {
        assert_eq!(
            total(
                &worker,
                target(revision, None, false),
                &[("Int32Value", aggregate)],
                None
            )
            .unwrap()
            .cells[0]
                .text,
            expected
        );
    }
    assert!(
        total(
            &worker,
            target(revision, None, false),
            &[
                ("Int32Value", Aggregate::Sum),
                ("Int32Value", Aggregate::Count)
            ],
            None
        )
        .is_err()
    );
}

#[test]
fn excluded_nonfinite_values_do_not_poison_totals_and_failures_keep_old_view() {
    let source = canonical_ordinary_wide_insert::source(InsertCase::FixedSpecial);
    let (_directory, path, _worker, revision) = opened(&source.after);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    let Response::FilteredData(filtered) = document
        .apply_field_filter(
            &FieldFilterTarget {
                document_revision: revision,
                table: "Items".into(),
                column: "Int32Value".into(),
                filter_id: None,
                enabled: false,
            },
            FieldFilterOperator::Equal,
            &["0".into()],
            None,
            &|| false,
        )
        .unwrap()
    else {
        panic!("qualified membership excludes final NaN");
    };
    assert_eq!(filtered.source_ordinals, [0, 1]);
    let specs = [TotalSpec {
        column: "DoubleValue".into(),
        aggregate: Aggregate::Sum,
    }];
    let active = target(revision, Some(&filtered.handle), true);
    let Response::Totals(totals) = document
        .totals(&active, &specs, Some(&filtered.handle), &|| false)
        .unwrap()
    else {
        panic!("excluded rows are not aggregate operands");
    };
    assert_eq!(totals.view_rows, 2);
    assert!(
        document
            .totals(&target(revision, None, false), &specs, None, &|| false)
            .is_err()
    );
    assert!(matches!(
        document.totals(&active, &specs, Some(&filtered.handle), &|| true),
        Err(Error::Cancelled)
    ));
    let Response::FilteredData(retained) = document
        .filter_page(&filtered.handle, 0, true, &|| false)
        .unwrap()
    else {
        panic!("failed and canceled totals preserve the old view");
    };
    assert_eq!(retained.source_ordinals, [0, 1]);
}
