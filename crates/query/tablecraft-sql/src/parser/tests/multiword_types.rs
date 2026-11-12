//! Only documented Access compound synonyms form one owned type declaration.
use crate::{
    ParseLimits, parse_query,
    query::{
        DataType, Query, Statement,
        definition::{AlterOperation, Definition, TableElement},
    },
};

fn declaration(query: &Query) -> &DataType {
    if let Some(parameter) = query.parameters.first() {
        return &parameter.data_type;
    }
    let Statement::Definition(definition) = &query.statement else {
        panic!("declaration");
    };
    match definition.as_ref() {
        Definition::CreateTable(table) => {
            let TableElement::Column(column) = &table.elements[0] else {
                panic!("column");
            };
            &column.data_type
        }
        Definition::AlterTable(table) => match &table.operation {
            AlterOperation::AddColumn(column) | AlterOperation::AlterColumn(column) => {
                &column.data_type
            }
            _ => panic!("column operation"),
        },
        _ => panic!("typed definition"),
    }
}

#[test]
fn documented_compound_type_names_retain_owned_spelling_and_arguments_in_every_declaration() {
    for alias in [
        "CHARACTER VARYING",
        "NATIONAL CHARACTER",
        "NATIONAL CHAR",
        "NATIONAL CHARACTER VARYING",
        "NATIONAL CHAR VARYING",
        "BINARY VARYING",
        "BIT VARYING",
    ] {
        for sql in [
            format!("PARAMETERS P {alias}(7); SELECT P"),
            format!("CREATE TABLE T (F {alias}(7))"),
            format!("ALTER TABLE T ADD COLUMN F {alias}(7)"),
            format!("ALTER TABLE T ALTER COLUMN F {alias}(7)"),
        ] {
            let parsed = parse_query(&sql.to_ascii_lowercase(), Default::default()).unwrap();
            assert_eq!(declaration(parsed.query()).name, alias);
            assert_eq!(declaration(parsed.query()).arguments, [7]);
            let normalized = parsed.query().to_string();
            let reparsed = parse_query(&normalized, Default::default()).unwrap();
            assert_eq!(parsed.query(), reparsed.query(), "{normalized}");
        }
    }
    let parsed = parse_query("PARAMETERS P BIT; SELECT P", Default::default()).unwrap();
    assert_eq!(declaration(parsed.query()).name, "BIT");
}

#[test]
fn unknown_or_quoted_compound_continuations_cannot_extend_a_type_into_another_clause() {
    for kind in [
        "NATIONAL",
        "NATIONAL LONG",
        "CHAR VARYING",
        "NATIONAL NCHAR",
        "CHARACTER [VARYING]",
        "NATIONAL [CHARACTER] VARYING",
        "[NATIONAL] CHAR",
        "NATIONAL CHARACTER [VARYING]",
        "BIT [VARYING]",
        "NATIONAL CHARACTER VARYING VARYING",
        "BINARY CHARACTER",
        "NATIONAL CHARACTER(7) VARYING",
    ] {
        let sql = format!("CREATE TABLE T (F {kind})");
        assert!(parse_query(&sql, Default::default()).is_err(), "{kind}");
    }
    assert!(
        parse_query(
            "CREATE TABLE T(F NATIONAL CHAR VARYING(7)); DROP TABLE Other",
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn compound_words_remain_inside_original_byte_token_and_depth_allowances() {
    let sql = "CREATE TABLE T (F NATIONAL CHARACTER VARYING(7))";
    for limits in [
        ParseLimits {
            max_bytes: sql.len() - 1,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 8,
            ..Default::default()
        },
        ParseLimits {
            max_depth: 1,
            ..Default::default()
        },
    ] {
        assert!(parse_query(sql, limits).is_err());
    }
    parse_query(sql, Default::default()).unwrap();
}
