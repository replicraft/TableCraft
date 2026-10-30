//! Source/expected images use literal final rows, never the default resolver.
mod api;
mod authority;

use super::{
    incrementing_autonumber::model as generated,
    model as original,
    ordinary_rows::{fixture, model::Row},
    passive_defaults::model as properties,
};
use crate::{RuleCheckStatus, check_table_constraints, read_table};
use properties::{PAGE, Property, field};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

struct Case {
    source: fixture::Case,
    final_row: Row,
    omitted: Vec<usize>,
}

fn source(code: u8, name: &str, expression: Option<&str>) -> fixture::Case {
    let mut case = fixture::source(code, true, false, fixture::Index::Alias, false);
    let mut score = vec![Property::boolean("AllowZeroLength", true)];
    if name == "Score"
        && let Some(expression) = expression
    {
        score.push(Property::text("DefaultValue", expression));
    }
    let mut groups = vec![
        field(
            "LaterText",
            vec![Property::boolean("AllowZeroLength", true)],
        ),
        field("Score", score),
    ];
    if name != "Score"
        && let Some(expression) = expression
    {
        let name = match name {
            "Earlier" => "Earlier",
            "Flag" => "Flag",
            "ID" => "ID",
            _ => panic!("unknown literal field"),
        };
        groups.push(field(
            name,
            vec![Property::text("DefaultValue", expression)],
        ));
    }
    properties::properties(&mut case.before, &groups);
    case
}

fn cases(code: u8) -> Vec<Case> {
    let mut result = Vec::new();
    for (expression, omitted, value) in [
        (Some("\"fallback\""), true, Some("fallback")),
        (Some("\"fallback\""), false, None),
        (Some("\"fallback\""), false, Some("mIxEd")),
        (None, true, None),
        (Some("\"\""), true, Some("")),
        (Some("Null"), true, None),
        (Some("UnresolvedDefaultFunction()"), false, Some("mIxEd")),
        (Some("Date()"), false, None),
        (Some("malformed( ;"), false, Some("explicit")),
    ] {
        let mut row = Row::rich(10);
        row.score = value.map(str::to_owned);
        result.push(Case {
            source: source(code, "Score", expression),
            final_row: row,
            omitted: if omitted { vec![13] } else { Vec::new() },
        });
    }

    let mut integer = Row::rich(10);
    integer.earlier = Some(42);
    result.push(Case {
        source: source(code, "Earlier", Some("42")),
        final_row: integer,
        omitted: vec![0],
    });
    let mut boolean = Row::rich(10);
    boolean.flag = false;
    result.push(Case {
        source: source(code, "Flag", Some("False")),
        final_row: boolean,
        omitted: vec![1],
    });
    let mut needed_elsewhere = source(code, "Earlier", Some("42"));
    properties::properties(
        &mut needed_elsewhere.before,
        &[
            field(
                "LaterText",
                vec![Property::boolean("AllowZeroLength", true)],
            ),
            field(
                "Score",
                vec![
                    Property::boolean("AllowZeroLength", true),
                    Property::text("DefaultValue", "UnresolvedDefaultFunction()"),
                ],
            ),
            field("Earlier", vec![Property::text("DefaultValue", "42")]),
        ],
    );
    let mut row = Row::rich(10);
    row.earlier = Some(42);
    row.score = Some("mIxEd".to_owned());
    result.push(Case {
        source: needed_elsewhere,
        final_row: row,
        omitted: vec![0],
    });
    result
}

fn checked(bytes: &[u8], rows: usize, aliases: usize) {
    original::checked(bytes, rows, aliases);
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    for field in report.fields() {
        let states = [field.required(), field.empty_text(), field.validation()];
        assert!(
            !states.contains(&RuleCheckStatus::Unsupported),
            "layout {}, field {}, required/empty/validation {states:?}",
            bytes[20],
            field.name()
        );
    }
}

fn refusal_sources(code: u8) -> Vec<Vec<u8>> {
    let mut sources = Vec::new();
    for expression in [
        "Date()",
        "Now()",
        "UnknownFunction()",
        "[Earlier]",
        "1; SELECT 2",
        "",
        "\"text\"",
    ] {
        sources.push(source(code, "Earlier", Some(expression)).before);
    }
    sources.push(source(code, "ID", Some("2")).before);
    sources.push(source(code, "Flag", None).before);
    sources.push(properties::ordinary(code, "Null", true).before);
    let mut validation = source(code, "ID", Some("-1"));
    properties::properties(
        &mut validation.before,
        &[
            field(
                "LaterText",
                vec![Property::boolean("AllowZeroLength", true)],
            ),
            field("Score", vec![Property::boolean("AllowZeroLength", true)]),
            field(
                "ID",
                vec![
                    Property::text("DefaultValue", "-1"),
                    Property::text("ValidationRule", ">0"),
                ],
            ),
        ],
    );
    sources.push(validation.before);
    sources
}

#[test]
fn qualify_omitted_default_sources_and_complete_literal_expected_before_resolver() {
    for code in [5, 6] {
        for (position, case) in cases(code).into_iter().enumerate() {
            eprintln!("layout {code}, ordinary positive source/expected {position}");
            let expected = fixture::expected(&case.source, std::slice::from_ref(&case.final_row));
            checked(&case.source.before, 3, 2);
            checked(&expected, 4, 2);
            assert_eq!(
                &expected[4 * PAGE..5 * PAGE],
                &case.source.before[4 * PAGE..5 * PAGE]
            );
            assert!(case.omitted.iter().all(|position| *position < 17));
            let snapshot = read_table(
                &mut Cursor::new(&expected),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(snapshot.rows()[3], case.final_row.values(true));
        }
        let refusals = refusal_sources(code);
        for (position, bytes) in refusals.iter().enumerate() {
            eprintln!("layout {code}, refusal source {position}");
            checked(bytes, 3, 2);
        }
        let validation = refusals.last().unwrap();
        let report = check_table_constraints(
            &mut Cursor::new(validation),
            CatalogScope::new(1),
            "Items",
            &mut original::budget(),
            &|| false,
        )
        .unwrap();
        let id = report
            .fields()
            .iter()
            .find(|field| field.name() == "ID")
            .unwrap();
        assert_eq!(id.validation(), RuleCheckStatus::Checked);
    }
}

#[test]
fn qualify_omitted_incrementing_companion_source_and_complete_expected_before_resolver() {
    for code in [5, 6] {
        let mut case = generated::source(code, generated::State::HighWater);
        properties::properties(
            &mut case.before,
            &[field("Score", vec![Property::text("DefaultValue", "99")])],
        );
        let added = generated::Row {
            earlier: 4,
            id: 261,
            score: Some(99),
        };
        let expected = generated::expected(&case, std::slice::from_ref(&added));
        checked(&case.before, 2, 4);
        checked(&expected, 3, 4);
        assert_eq!(
            &expected[4 * PAGE..5 * PAGE],
            &case.before[4 * PAGE..5 * PAGE]
        );
    }
}
