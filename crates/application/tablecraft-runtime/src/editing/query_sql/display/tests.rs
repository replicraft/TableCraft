use super::*;
use std::cell::Cell;
use tablecraft_sql::parse_query;

#[test]
fn reconstructed_syntax_round_trips_without_executing_its_external_source() {
    let parsed = parse_query(
        "PARAMETERS p LONG; SELECT [Name], 'it''s data' AS [Label] FROM [Items] IN 'never-open.accdb' WHERE [ID] >= p",
        ParseLimits::default(),
    )
    .unwrap();
    let mut output = Presentation::new(&|| false);
    let text = query(parsed.query(), &mut output).unwrap();
    assert_eq!(
        parse_query(&text, ParseLimits::default()).unwrap().query(),
        parsed.query()
    );
}

#[test]
fn formatting_checks_byte_limit_and_preserves_the_first_failure() {
    let mut presentation = Presentation::new(&|| false);
    let mut output = Output {
        presentation: &mut presentation,
        text: String::new(),
        error: None,
    };
    output
        .write_str(&"x".repeat(ParseLimits::MAX_BYTES))
        .unwrap();
    assert!(output.write_str("é").is_err());
    assert!(matches!(output.error, Some(Error::SqlLimit)));
    assert_eq!(output.text.len(), ParseLimits::MAX_BYTES);
    assert!(output.write_str("ignored").is_err());
    assert_eq!(output.text.len(), ParseLimits::MAX_BYTES);
}

#[test]
fn cancellation_during_formatting_returns_no_published_prefix() {
    let parsed = parse_query(
        "SELECT [ID] FROM [Items] WHERE [ID] >= 2",
        ParseLimits::default(),
    )
    .unwrap();
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() > 3
    };
    let mut output = Presentation::new(&cancelled);
    assert!(matches!(
        query(parsed.query(), &mut output),
        Err(Error::Cancelled)
    ));
    assert!(checks.get() > 3);
}
