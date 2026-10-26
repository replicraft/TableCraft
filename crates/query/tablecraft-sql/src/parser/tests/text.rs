//! Access string delimiters are syntax, not identifier quotes or a VBA wrapper.
//! References and the scope of normalization are recorded in docs/query/SQL_PARSER.md.

use super::*;

#[test]
fn double_quotes_produce_text_and_preserve_embedded_quotes() {
    let sql = r#"SELECT * FROM Samples WHERE Label = "He said ""Sam's""""#;
    let result = parsed(sql);
    assert_eq!(result.source(), sql);
    let Expression::Binary { right, .. } = result.query().selection.as_ref().unwrap() else {
        panic!("comparison expected");
    };
    assert_eq!(**right, Expression::Text("He said \"Sam's\"".to_owned()));
    assert_eq!(
        result.query().to_string(),
        "SELECT * FROM [Samples] WHERE ([Label] = 'He said \"Sam''s\"');"
    );
    round_trip(sql);
}

#[test]
fn double_quotes_do_not_become_identifier_delimiters() {
    let text = parsed(r#"SELECT * FROM Samples WHERE Label = "Code""#);
    let field = parsed("SELECT * FROM Samples WHERE Label = [Code]");
    assert_ne!(text.query(), field.query());
    for sql in [
        r#"SELECT Code AS "Public Code" FROM Samples"#,
        r#"SELECT * FROM "Samples""#,
        r#"SELECT * FROM Samples AS "s""#,
        r#"SELECT * FROM Samples WHERE "s".Code = 1"#,
    ] {
        assert!(
            parse_select(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}

#[test]
fn delimiters_and_lexical_markers_inside_text_are_not_sql_structure() {
    for sql in [
        r#"SELECT * FROM Samples WHERE Label = """#,
        r#"SELECT * FROM Samples WHERE Label = "Été 東京""#,
        r##"SELECT * FROM Samples WHERE Label = "#date#; -- /* () != ==""##,
        r#"SELECT * FROM Samples WHERE Label = "C:\samples\new""#,
        "SELECT * FROM Samples WHERE Label = \"line one\nline two\"",
        r#"SELECT * FROM Samples WHERE Label = "'single'" AND Code = 1"#,
    ] {
        round_trip(sql);
    }
    for sql in [
        "SELECT * FROM Samples WHERE Label = \"unfinished",
        "SELECT * FROM Samples WHERE Label = \"unfinished\"\"",
        "SELECT * FROM Samples WHERE Label = \"finished\" extra",
    ] {
        assert!(
            parse_select(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
