use super::queries::query_round_trip;
use super::*;
use crate::query::{ReadBody, Statement};
use proptest::prelude::*;

#[test]
fn bracketed_punctuation_remains_one_unqualified_prompt_in_declarations_and_expressions() {
    for name in [
        "Enter No. of items",
        "P.Q",
        "Floor!",
        "Ask!Now",
        "Forms!My Form!ID",
        "Rate `value` (%)",
        "What; SQL?",
        "What) OR (True",
        "é. nombre",
    ] {
        let sql = format!("PARAMETERS [{name}] LONG; SELECT [{name}]");
        let parsed = query_round_trip(&sql);
        assert_eq!(parsed.query().parameters[0].name.as_str(), name);
        let Statement::Read(read) = &parsed.query().statement else {
            panic!()
        };
        let ReadBody::Select(select) = &read.body else {
            panic!()
        };
        assert!(
            matches!(&select.projection[0],Projection::Field{field,alias:None} if field.qualifier.is_none()&&field.name.as_str()==name)
        );
    }
    query_round_trip("SELECT [Enter No. of items]+1");
    query_round_trip("SELECT Forms![Search Form].Code FROM Items");
}

#[test]
fn prompt_punctuation_does_not_relax_objects_qualifiers_bang_members_or_callables() {
    for name in ["P.Q", "Ask!Now", "P`Q"] {
        for sql in [
            format!("SELECT 1 FROM [{name}]"),
            format!("SELECT 1 AS [{name}]"),
            format!("SELECT [{name}].Id"),
            format!("SELECT [{name}]!Id"),
            format!("SELECT ([{name}]).Id"),
            format!("SELECT [{name}].* FROM Items"),
            format!("SELECT [{name}](1)"),
            format!("SELECT t![{name}] FROM Items AS t"),
            format!("CREATE TABLE Items ([{name}] LONG)"),
        ] {
            assert!(
                crate::parse_query(&sql, Default::default()).is_err(),
                "{sql}"
            );
        }
    }
    for name in ["", "P\nQ", "P\0Q", "[P]", "P] AS [Hidden"] {
        let sql = format!("PARAMETERS [{name}] LONG; SELECT 1");
        assert!(
            crate::parse_query(&sql, Default::default()).is_err(),
            "{sql}"
        );
    }
    assert!(crate::parse_query("PARAMETERS P.Q LONG; SELECT 1", Default::default()).is_err());
}

#[test]
fn generated_heading_punctuation_is_a_literal_qualified_field_reference() {
    for name in ["Sales.Q1", "Sales!Q2", "Rate `value`"] {
        let parsed = query_round_trip(&format!("SELECT p.[{name}] FROM Piv AS p"));
        let Statement::Read(read) = &parsed.query().statement else {
            panic!()
        };
        let ReadBody::Select(select) = &read.body else {
            panic!()
        };
        assert!(
            matches!(&select.projection[0],Projection::Field{field,alias:None} if field.qualifier.as_ref().map(|q|q.as_str())==Some("p")&&field.name.as_str()==name)
        );
        assert!(crate::parse_query(&format!("SELECT p.[{name}](1)"), Default::default()).is_err());
    }
}

#[test]
fn prompt_text_still_obeys_parser_byte_and_token_limits() {
    let sql = "PARAMETERS [Enter No. of items] LONG; SELECT [Enter No. of items]";
    for limits in [
        ParseLimits {
            max_bytes: sql.len() - 1,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 3,
            ..Default::default()
        },
    ] {
        assert_eq!(
            crate::parse_query(sql, limits).unwrap_err().kind,
            ParseErrorKind::Limit
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn prompt_symbols_round_trip_without_becoming_sql_structure(name in "[a-zA-Z0-9 .!`?;:/()#%_+-]{1,40}") {
        let sql=format!("PARAMETERS [{name}] LONG; SELECT [{name}]");
        let parsed=query_round_trip(&sql);
        prop_assert_eq!(parsed.query().parameters[0].name.as_str(),name);
    }
}
