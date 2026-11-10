//! Entered common filters bind native fields and preserve query operand precision.
use super::{page, request, sort};
use crate::tests::stored::opened;
use crate::{
    Command, Error, FieldFilterDescription, FieldFilterOperator as Operator,
    FieldFilterPatternMode, FieldFilterTarget, FilterError, FilterHandle, FilteredData, InputHint,
    Response, SortSpec, Worker,
};
use tablecraft_expressions::EvalError;
use tablecraft_testkit::fixtures::access::{
    canonical_fixed_creation as fixed, canonical_variable_creation as variable,
    database_properties, field_presentation, mutation_fixture,
};

fn target(
    revision: u64,
    column: &str,
    base: Option<&FilterHandle>,
    enabled: bool,
) -> FieldFilterTarget {
    FieldFilterTarget {
        document_revision: revision,
        table: "Items".into(),
        column: column.into(),
        filter_id: base.map(|handle| handle.target().filter_id),
        enabled,
    }
}

fn describe(worker: &Worker, target: FieldFilterTarget) -> Result<FieldFilterDescription, Error> {
    let Response::FieldFilterDescription(description) =
        request(worker, Command::DescribeFieldFilter { target })?
    else {
        panic!("expected informational native field metadata");
    };
    Ok(description)
}

fn entered(
    worker: &Worker,
    target: FieldFilterTarget,
    operator: Operator,
    inputs: &[&str],
    base: Option<&FilterHandle>,
) -> Result<FilteredData, Error> {
    let Response::FilteredData(data) = request(
        worker,
        Command::ApplyFieldFilter {
            target,
            operator,
            inputs: inputs.iter().map(|input| (*input).into()).collect(),
            base: base.cloned(),
        },
    )?
    else {
        panic!("expected complete native common-filter publication");
    };
    Ok(data)
}

#[test]
fn description_needs_no_rows_and_does_not_grant_apply_permission() {
    let bytes = mutation_fixture::with_rows(&[]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let field = target(revision, "Score", None, false);
    let description = describe(&worker, field.clone()).unwrap();
    assert_eq!(description.input_hint, InputHint::Decimal);
    assert_eq!(description.pattern_mode, None);
    assert!(description.operators.contains(&Operator::Between));
    let empty = entered(&worker, field, Operator::Greater, &["-0.5"], None).unwrap();
    assert_eq!((empty.source_rows, empty.matched_rows), (0, 0));
    assert_eq!(empty.columns.len(), 2);
    assert!(empty.source_ordinals.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), bytes);

    let mut corrupt = mutation_fixture::database();
    const PAGE_BYTES: usize = 4096;
    const USER_DATA_PAGE: usize = 12;
    corrupt[USER_DATA_PAGE * PAGE_BYTES] = 0xff;
    let (_directory, path, worker, revision) = opened(&corrupt);
    let field = target(revision, "Score", None, false);
    assert!(describe(&worker, field.clone()).is_ok());
    assert!(entered(&worker, field, Operator::Greater, &["0"], None).is_err());
    assert_eq!(std::fs::read(path).unwrap(), corrupt);
}

#[test]
fn fractional_reversed_ranges_replace_same_field_and_inherit_sort_across_modes() {
    let rows: Vec<_> = (0..105).map(|index| (index + 1, index)).collect();
    let (_directory, _path, worker, revision) = opened(&mutation_fixture::with_rows(&rows));
    let first = entered(
        &worker,
        target(revision, "Score", None, false),
        Operator::Between,
        &["20.75", "10.25"],
        None,
    )
    .unwrap();
    assert_eq!(first.source_rows, 105);
    assert_eq!(first.source_ordinals, (11..=20).collect::<Vec<_>>());
    let sorted = sort(
        &worker,
        revision,
        Some(&first.handle),
        true,
        Some(SortSpec {
            column: "Score".into(),
            descending: true,
        }),
    )
    .unwrap();
    let both = entered(
        &worker,
        target(revision, "Id", Some(&sorted.handle), true),
        Operator::Greater,
        &["15"],
        Some(&sorted.handle),
    )
    .unwrap();
    assert_eq!(both.source_ordinals, (15..=20).rev().collect::<Vec<_>>());
    assert_eq!(both.filter_count, 2);
    let replaced = entered(
        &worker,
        target(revision, "Score", Some(&first.handle), true),
        Operator::Less,
        &["0.5"],
        Some(&first.handle),
    )
    .unwrap();
    assert_eq!(replaced.source_ordinals, [0]);
    assert_eq!(replaced.filter_count, 1);
    let off = page(&worker, &both.handle, 0, false).unwrap();
    let fresh = entered(
        &worker,
        target(revision, "Score", Some(&off.handle), false),
        Operator::LessOrEqual,
        &["1.5"],
        Some(&off.handle),
    )
    .unwrap();
    assert_eq!(fresh.source_ordinals, [1, 0]);
    assert_eq!(fresh.filter_count, 1);
    assert_eq!(fresh.sort, sorted.sort);
    assert_eq!(
        page(&worker, &first.handle, 0, true)
            .unwrap()
            .source_ordinals
            .len(),
        10
    );
    let outside = entered(
        &worker,
        target(revision, "Score", None, false),
        Operator::NotBetween,
        &["10.25", "20.75"],
        None,
    )
    .unwrap();
    assert_eq!(outside.matched_rows, 95);
    assert_eq!(outside.source_ordinals[10..13], [10, 21, 22]);
}

#[test]
fn currency_bounds_are_not_write_rounded_and_float_canonical_guard_is_explicit() {
    let bytes = field_presentation::score64(&[], 5, &[12345, 12346]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let field = target(revision, "Score", None, false);
    assert_eq!(
        describe(&worker, field.clone()).unwrap().input_hint,
        InputHint::Decimal
    );
    assert!(
        entered(&worker, field.clone(), Operator::Equal, &["1.23451"], None)
            .unwrap()
            .source_ordinals
            .is_empty()
    );
    assert_eq!(
        entered(&worker, field, Operator::Greater, &["1.23451"], None)
            .unwrap()
            .source_ordinals,
        [1]
    );

    let bytes = field_presentation::score64(&[], 7, &[0.1_f64.to_bits(), 1.0_f64.to_bits()]);
    let (_directory, _path, worker, revision) = opened(&bytes);
    let ordinary = entered(
        &worker,
        target(revision, "Score", None, false),
        Operator::Equal,
        &["0.1000"],
        None,
    )
    .unwrap();
    assert_eq!(ordinary.source_ordinals, [0]);
    let source = fixed::source(fixed::Case::Nullable);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    let field = target(revision, "SingleValue", None, false);
    assert_eq!(
        entered(&worker, field.clone(), Operator::Equal, &["0.5000"], None)
            .unwrap()
            .source_ordinals,
        [1]
    );
    assert!(matches!(
        entered(&worker, field, Operator::Equal, &["1.00000001"], None),
        Err(Error::Conversion(EvalError::PrecisionLoss))
    ));
    assert!(matches!(
        entered(
            &worker,
            target(revision, "Int32Value", None, false),
            Operator::Equal,
            &["1.00000000000000000000000000001"],
            None
        ),
        Err(Error::Conversion(EvalError::PrecisionLoss))
    ));
    assert!(
        entered(
            &worker,
            target(revision, "Int32Value", None, false),
            Operator::Equal,
            &["1,25"],
            None
        )
        .is_err()
    );
    assert_eq!(
        entered(
            &worker,
            target(revision, "Int32Value", None, false),
            Operator::NotEqual,
            &["200"],
            None
        )
        .unwrap()
        .source_ordinals,
        [2]
    );
}

#[test]
fn date_bounds_are_typed_inclusive_reversible_and_not_regional_input() {
    let bytes = field_presentation::score64(
        &[],
        8,
        &[0.0_f64.to_bits(), 1.0_f64.to_bits(), 2.0_f64.to_bits()],
    );
    let (_directory, _path, worker, revision) = opened(&bytes);
    let field = target(revision, "Score", None, false);
    assert_eq!(
        describe(&worker, field.clone()).unwrap().input_hint,
        InputHint::DateTimeIso
    );
    assert_eq!(
        entered(
            &worker,
            field.clone(),
            Operator::Between,
            &["1899-12-31T00:00:00", "1899-12-30T00:00:00"],
            None
        )
        .unwrap()
        .source_ordinals,
        [0, 1]
    );
    assert_eq!(
        entered(
            &worker,
            field.clone(),
            Operator::NotBetween,
            &["1899-12-30T00:00:00", "1899-12-31T00:00:00"],
            None
        )
        .unwrap()
        .source_ordinals,
        [2]
    );
    assert!(entered(&worker, field, Operator::Equal, &["12/31/1899"], None).is_err());
}

#[test]
fn native_text_patterns_compose_without_escaping_and_null_is_not_empty() {
    let source = variable::source(variable::Case::Unicode);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    let field = target(revision, "TextValue", None, false);
    assert_eq!(
        describe(&worker, field.clone()).unwrap().pattern_mode,
        Some(FieldFilterPatternMode::Ansi89)
    );
    for (operator, pattern, expected) in [
        (Operator::Equal, "abc*", vec![2]),
        (Operator::BeginsWith, "ABC", vec![2]),
        (Operator::Contains, "雪", vec![0]),
        (Operator::EndsWith, "gh", vec![2]),
        (Operator::NotContains, "雪", vec![1, 2]),
    ] {
        assert_eq!(
            entered(&worker, field.clone(), operator, &[pattern], None)
                .unwrap()
                .source_ordinals,
            expected
        );
    }
    let source = variable::source(variable::Case::NullAndEmpty);
    let (_directory, _path, worker, revision) = opened(&source.bytes);
    let field = target(revision, "TextValue", None, false);
    for pattern in ["*", ""] {
        assert_eq!(
            entered(&worker, field.clone(), Operator::Equal, &[pattern], None)
                .unwrap()
                .source_ordinals,
            [1, 3]
        );
    }
    assert!(
        entered(&worker, field.clone(), Operator::NotEqual, &[""], None)
            .unwrap()
            .source_ordinals
            .is_empty()
    );
    assert_eq!(
        entered(&worker, field, Operator::IsNull, &[], None)
            .unwrap()
            .source_ordinals,
        [0, 2]
    );
}

#[test]
fn malformed_operands_and_stale_sources_leave_previous_native_view_usable() {
    let bytes = mutation_fixture::database();
    let (_directory, _path, worker, revision) = opened(&bytes);
    let first = entered(
        &worker,
        target(revision, "Score", None, false),
        Operator::Greater,
        &["15"],
        None,
    )
    .unwrap();
    let field = target(revision, "Score", Some(&first.handle), true);
    for (operator, inputs, expected) in [
        (Operator::Between, vec!["10"], FilterError::InvalidOperands),
        (
            Operator::Contains,
            vec!["10"],
            FilterError::UnsupportedOperator,
        ),
    ] {
        let error = entered(
            &worker,
            field.clone(),
            operator,
            &inputs,
            Some(&first.handle),
        )
        .err()
        .expect("malformed or unsupported operands must refuse");
        assert!(matches!(
            (error, expected),
            (
                Error::Filter(FilterError::InvalidOperands),
                FilterError::InvalidOperands
            ) | (
                Error::Filter(FilterError::UnsupportedOperator),
                FilterError::UnsupportedOperator
            )
        ));
    }

    let oversized = "1".repeat(65_537);
    let rejected = entered(
        &worker,
        field.clone(),
        Operator::Equal,
        &[&oversized],
        Some(&first.handle),
    )
    .err()
    .expect("an out-of-range numeric operand must remain invalid");
    assert!(!matches!(rejected, Error::Filter(FilterError::InputLimit)));

    let (_other_directory, _other_path, other, other_revision) = opened(&bytes);
    assert_eq!(other_revision, revision);
    assert!(matches!(
        entered(&other, field, Operator::Equal, &["20"], Some(&first.handle)),
        Err(Error::Filter(FilterError::StaleHandle))
    ));
    assert_eq!(
        page(&worker, &first.handle, 0, true)
            .unwrap()
            .source_ordinals,
        [1, 2]
    );
}

// The catalog Name descriptor is independently authored as General text, rather
// than allowing the fixture's unknown text policy to acquire a binary fallback.
fn query_mode_source(mode: i32) -> Vec<u8> {
    let properties = database_properties::query_modes(&[(1, 4, &mode.to_le_bytes())]);
    let mut bytes = database_properties::database(Some(&properties));
    const PAGE_BYTES: usize = 4096;
    const CATALOG_DEFINITION_PAGE: usize = 2;
    const DESCRIPTOR_START: usize = 63;
    const DESCRIPTOR_BYTES: usize = 25;
    const NAME_POSITION: usize = 2;
    const TEXT_SORT_OFFSET: usize = 11;
    let offset = CATALOG_DEFINITION_PAGE * PAGE_BYTES
        + DESCRIPTOR_START
        + NAME_POSITION * DESCRIPTOR_BYTES
        + TEXT_SORT_OFFSET;
    bytes[offset..offset + 4].copy_from_slice(&[9, 4, 0, 0]);
    bytes
}

#[test]
fn entered_patterns_rebind_database_ansi_mode_and_unknown_mode_refuses() {
    for (mode, expected_mode, pattern) in [
        (0, FieldFilterPatternMode::Ansi89, "I*"),
        (1, FieldFilterPatternMode::Ansi92, "I%"),
    ] {
        let bytes = query_mode_source(mode);
        let (_directory, _path, worker, revision) = opened(&bytes);
        let mut field = target(revision, "Name", None, false);
        field.table = "MSysObjects".into();
        assert_eq!(
            describe(&worker, field.clone()).unwrap().pattern_mode,
            Some(expected_mode)
        );
        let first = entered(&worker, field.clone(), Operator::Equal, &[pattern], None).unwrap();
        assert_eq!(first.source_ordinals, [2]);
        let opposite = if mode == 0 { "I%" } else { "I*" };
        assert!(
            entered(&worker, field, Operator::Equal, &[opposite], None)
                .unwrap()
                .source_ordinals
                .is_empty()
        );
        let other_bytes = query_mode_source(1 - mode);
        let (_other_directory, _other_path, other, other_revision) = opened(&other_bytes);
        let Response::FilteredData(rebound) = request(
            &other,
            Command::ReapplyFilter {
                definition: first.handle.definition(),
                document_revision: other_revision,
                enabled: true,
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh pattern definition reapplication");
        };
        assert!(rebound.source_ordinals.is_empty());
    }
    let (_directory, _path, worker, revision) = opened(&query_mode_source(2));
    let mut field = target(revision, "Name", None, false);
    field.table = "MSysObjects".into();
    assert!(matches!(
        describe(&worker, field.clone()),
        Err(Error::Filter(FilterError::UnknownPatternMode))
    ));
    assert!(matches!(
        entered(&worker, field, Operator::Equal, &["I*"], None),
        Err(Error::Filter(FilterError::UnknownPatternMode))
    ));
    assert_eq!(
        entered(
            &worker,
            target(revision, "Score", None, false),
            Operator::Greater,
            &["15"],
            None
        )
        .unwrap()
        .source_ordinals,
        [1, 2]
    );
}
#[test]
fn large_typed_text_operands_compare_without_a_shared_utf8_byte_ceiling() {
    let source = variable::source(variable::Case::NullAndEmpty);
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let text = "雪".repeat(30_000);
    let filtered = entered(
        &worker,
        target(revision, "TextValue", None, false),
        Operator::Equal,
        &[&text],
        None,
    )
    .unwrap();
    assert!(filtered.source_ordinals.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), source.bytes);
}
