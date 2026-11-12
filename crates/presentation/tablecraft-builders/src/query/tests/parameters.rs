//! Declaration ownership survives grid import/build without host type parsing.
use super::{draft, metadata};
use crate::query::{
    DesignExpression, MAX_DESIGN_TEXT_BYTES, QueryDesignError, build, build_ast, draft_text_bytes,
    import, parse_parameters, render_parameters,
};

use tablecraft_sql::{ParseErrorKind, ParseLimits, parse_expression_strict};

#[test]
fn declared_inputs_roundtrip_without_replacing_matching_native_fields() {
    let mut draft = draft();
    draft.parameters = parse_parameters("PARAMETERS [Threshold] LONG, [Amount] BYTE;").unwrap();
    draft.columns[1].criteria = vec!["> [Threshold]".into()];
    let sql = build(&draft, &metadata()).unwrap().sql;
    assert!(sql.starts_with("PARAMETERS [Threshold] LONG, [Amount] BYTE;"));
    assert!(sql.contains("[Customers].[Amount] > [Threshold]"));
    let imported = import(&sql, &metadata()).unwrap();
    assert_eq!(imported.draft.parameters, draft.parameters);
    assert_eq!(imported.sql, sql);
    draft.columns[1].expression = DesignExpression::Expression("[Threshold]".into());
    assert!(build_ast(&draft, &metadata()).is_ok());
    draft.columns[1].expression = DesignExpression::Expression("[Missing]".into());
    assert!(build_ast(&draft, &metadata()).is_err());
}

#[test]
fn declaration_text_uses_exact_owner_types_and_refuses_extra_statements() {
    let parameters =
        parse_parameters("PARAMETERS [minimum value] LONG, [Prefix] TEXT(20);").unwrap();
    let text = render_parameters(&parameters).unwrap();
    assert_eq!(parse_parameters(&text).unwrap(), parameters);
    assert_eq!(render_parameters(&[]).unwrap(), "");
    assert!(parse_parameters("").unwrap().is_empty());
    for invalid in [
        "SELECT 1;",
        "PARAMETERS [Min] LONG; SELECT 2;",
        "PARAMETERS [Min] LONG; DELETE FROM Customers;",
        "PARAMETERS [Min] LONG, [min] LONG;",
    ] {
        assert!(parse_parameters(invalid).is_err(), "{invalid}");
    }
    let mut invalid = parameters;
    invalid[0].data_type.name = "LONG; DELETE FROM Customers;".into();
    assert!(render_parameters(&invalid).is_err());
}

#[test]
fn text_criterion_parser_limits_are_not_reinterpreted_as_literal_text() {
    let deep = std::iter::repeat_n("1", ParseLimits::MAX_DEPTH + 10)
        .collect::<Vec<_>>()
        .join("+");
    let tokens = std::iter::repeat_n("word", ParseLimits::MAX_TOKENS + 1)
        .collect::<Vec<_>>()
        .join(" ");
    for text in [deep, tokens] {
        assert!(matches!(
            parse_expression_strict(&text, ParseLimits::default()),
            Err(error) if error.kind == ParseErrorKind::Limit
        ));
        let mut draft = draft();
        draft.columns[0].criteria = vec![text];
        assert!(draft_text_bytes(&draft).unwrap() < MAX_DESIGN_TEXT_BYTES);
        assert!(matches!(
            build(&draft, &metadata()),
            Err(QueryDesignError::Parse(error)) if error.kind == ParseErrorKind::Limit
        ));
        assert!(!draft.columns[0].criteria[0].is_empty());
    }
}

#[test]
fn a_wildcard_does_not_hide_the_operand_of_an_imported_filter_or_sort() {
    let sql = "SELECT Customers.* FROM Customers WHERE Customers.ID>0 ORDER BY Customers.ID";
    let imported = import(sql, &metadata()).unwrap();
    assert_eq!(
        build(&imported.draft, &metadata()).unwrap().sql,
        imported.sql
    );
    assert!(
        imported
            .draft
            .columns
            .iter()
            .any(|column| !column.show && !column.criteria.is_empty())
    );
}
