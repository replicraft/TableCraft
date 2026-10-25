//! Exact per-leaf selectors use existing bracket decoding, not Name validation.
use super::*;
use crate::select::{Table, TableSource};

fn table(source: &TableSource) -> &Table {
    let TableSource::Table(table) = source else {
        panic!("expected a named source");
    };
    table
}

#[test]
fn per_table_database_selectors_preserve_case_paths_and_owned_serialization() {
    for database in [
        "remote-token",
        "Remote-TOKEN",
        "C:\\temp\\Database1.accdb",
        "opaque ' é [x]; -- token",
        "SELECT .!` name",
    ] {
        let escaped = database.replace(']', "]]");
        let sql = format!("SELECT r.ID FROM [{escaped}].[Remote Table] AS r");
        let parsed = parsed(&sql);
        let source = table(&parsed.query().from[0]);
        assert_eq!(source.database.as_deref(), Some(database));
        assert_eq!(source.name.as_str(), "Remote Table");
        assert_eq!(source.alias.as_ref().unwrap().as_str(), "r");
        round_trip(&sql);
    }
}

#[test]
fn joins_products_and_global_in_keep_each_selector_on_its_written_leaf() {
    let query = parsed(concat!(
        "SELECT l.ID FROM Local l INNER JOIN [remote-token].Remote r ",
        "ON l.ID=r.ID,[second].Other s IN 'global-default'",
    ));
    assert_eq!(
        query.query().external.as_ref().unwrap().database,
        "global-default"
    );
    let TableSource::Join { left, right, .. } = &query.query().from[0] else {
        panic!("expected a join");
    };
    assert_eq!(table(left).database, None);
    assert_eq!(table(right).database.as_deref(), Some("remote-token"));
    assert_eq!(
        table(&query.query().from[1]).database.as_deref(),
        Some("second")
    );
    round_trip(query.source());
}

#[test]
fn nested_selectors_do_not_change_enclosing_or_sibling_source_structure() {
    let sql = concat!(
        "SELECT (SELECT Count(*) FROM [inner-token].Remote) AS N ",
        "FROM Local l INNER JOIN (SELECT ID FROM [derived-token].Remote) r ON l.ID=r.ID",
    );
    round_trip(sql);
    let query = parsed(sql);
    assert_eq!(query.query().external, None);
    let TableSource::Join { left, right, .. } = &query.query().from[0] else {
        panic!("expected a join");
    };
    assert_eq!(table(left).database, None);
    assert!(matches!(right.as_ref(), TableSource::Derived { .. }));
}

#[test]
fn selector_admission_does_not_relax_names_or_add_other_quote_and_path_forms() {
    for sql in [
        "SELECT * FROM [].Remote",
        "SELECT * FROM [bad\nselector].Remote",
        "SELECT * FROM [bad\0selector].Remote",
        "SELECT * FROM [a].[b].[Remote]",
        "SELECT * FROM 'remote-token'.Remote",
        "SELECT * FROM `remote-token`.Remote",
        "SELECT * FROM [remote-token].SELECT",
        "SELECT * FROM [remote-token].[bad.name]",
        "SELECT * FROM [remote-token].Remote AS [bad.alias]",
    ] {
        assert!(parse_select(sql, ParseLimits::default()).is_err(), "{sql}");
    }
    round_trip("SELECT * FROM [remote-token].[SELECT] AS r");
}

#[test]
fn long_selectors_share_existing_syntax_byte_and_token_limits() {
    let selector = "x".repeat(1024);
    let sql = format!("SELECT * FROM [{selector}].Remote");
    let byte_limits = ParseLimits {
        max_bytes: 64,
        ..Default::default()
    };
    assert_eq!(
        parse_select(&sql, byte_limits).unwrap_err().kind,
        ParseErrorKind::Limit
    );

    let token_limits = ParseLimits {
        max_tokens: 4,
        ..Default::default()
    };
    assert_eq!(
        parse_select(&sql, token_limits).unwrap_err().kind,
        ParseErrorKind::Limit
    );
}
