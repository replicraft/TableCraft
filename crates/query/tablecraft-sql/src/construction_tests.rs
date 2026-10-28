use crate::{
    ParseErrorKind, ParseLimits, parse_query,
    query::{Query, ReadBody, Statement},
    select::{Name, Projection, Select},
};

#[test]
fn owned_names_preserve_delimited_contents_and_refuse_delimiters() {
    for contents in ["Order Details", "SELECT", "☃"] {
        let name = Name::from_identifier(contents, ParseLimits::default()).unwrap();
        let text = format!("SELECT 0 AS [{contents}]");
        let parsed = parse_query(&text, ParseLimits::default())
            .unwrap()
            .into_query();
        assert_eq!(name.as_str(), contents);
        let Statement::Read(read) = parsed.statement else {
            panic!()
        };
        let ReadBody::Select(select) = read.body else {
            panic!()
        };
        let [
            Projection::Expression {
                alias: Some(alias), ..
            },
        ] = select.projection.as_slice()
        else {
            panic!()
        };
        assert_eq!(&name, alias);
    }

    for contents in ["", "a]b", "a[b", "a\nb", "\0"] {
        assert_eq!(
            Name::from_contents(contents, ParseLimits::default())
                .unwrap_err()
                .kind,
            ParseErrorKind::Syntax,
        );
    }
}

#[test]
fn prompt_punctuation_does_not_become_an_object_identifier() {
    for contents in ["a.b!c", "Enter a value!", "a`b"] {
        let name = Name::from_contents(contents, ParseLimits::default()).unwrap();
        let parsed = parse_query(
            &format!("PARAMETERS [{contents}] LONG; SELECT 0"),
            ParseLimits::default(),
        )
        .unwrap()
        .into_query();
        assert_eq!(parsed.parameters[0].name, name);
        assert_eq!(
            Name::from_identifier(contents, ParseLimits::default())
                .unwrap_err()
                .kind,
            ParseErrorKind::Unsupported,
        );
    }
}

#[test]
fn owned_names_check_limits_before_copying() {
    let limits = ParseLimits {
        max_bytes: 3,
        ..ParseLimits::default()
    };
    assert!(Name::from_contents("abc", limits).is_ok());
    assert_eq!(
        Name::from_contents("abcd", limits).unwrap_err().kind,
        ParseErrorKind::Limit
    );
    let limits = ParseLimits {
        max_tokens: 0,
        ..limits
    };
    assert_eq!(
        Name::from_contents("a", limits).unwrap_err().kind,
        ParseErrorKind::Limit
    );
    let excessive = "x".repeat(ParseLimits::MAX_BYTES + 1);
    let limits = ParseLimits {
        max_bytes: usize::MAX,
        ..ParseLimits::default()
    };
    assert_eq!(
        Name::from_contents(&excessive, limits).unwrap_err().kind,
        ParseErrorKind::Limit
    );
}

#[test]
fn select_carrier_moves_final_clauses_without_cloning_syntax() {
    let expected = parse_query(
        "SELECT [A] AS [Output] FROM [Items] ORDER BY [A] DESC WITH OWNERACCESS OPTION",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let Statement::Read(read) = expected.clone().statement else {
        panic!()
    };
    let ReadBody::Select(mut select) = read.body else {
        panic!()
    };
    select.order_by = read.order_by;
    select.owner_access = read.owner_access;
    assert_eq!(Query::from_select(*select), expected);
}

#[test]
fn projection_and_crosstab_carriers_preserve_owned_models() {
    let expected = parse_query("SELECT [A] FROM [Items]", ParseLimits::default())
        .unwrap()
        .into_query();
    let Statement::Read(read) = expected.clone().statement else {
        panic!()
    };
    let ReadBody::Select(select) = read.body else {
        panic!()
    };
    assert_eq!(
        Query::from_select(Select::from_projection(select.projection, select.from)),
        expected,
    );

    let expected = parse_query(
        "TRANSFORM Count(*) SELECT [A] FROM [Items] GROUP BY [A] PIVOT [B]",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let Statement::Crosstab(crosstab) = expected.clone().statement else {
        panic!()
    };
    assert_eq!(Query::from_crosstab(*crosstab), expected);
}
