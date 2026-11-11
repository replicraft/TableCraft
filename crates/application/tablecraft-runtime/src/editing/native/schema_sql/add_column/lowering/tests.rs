//! SQL absence remains absent before a native declared ADD can be planned.
use super::*;
use tablecraft_sql::query::{
    Statement,
    definition::{AlterOperation, Definition},
};
use tablecraft_sql::{ParseLimits, parse_query};

fn column(sql: &str) -> Column {
    let query = parse_query(sql, ParseLimits::default())
        .unwrap()
        .into_query();
    let Statement::Definition(definition) = query.statement else {
        panic!("expected definition");
    };
    let Definition::AlterTable(table) = *definition else {
        panic!("expected ALTER TABLE");
    };
    let AlterOperation::AddColumn(column) = table.operation else {
        panic!("expected ADD COLUMN");
    };
    column
}

#[test]
fn currency_aliases_keep_every_unrequested_policy_absent() {
    for kind in ["CURRENCY", "MONEY"] {
        let syntax = column(&format!("ALTER TABLE Table1 ADD COLUMN Amount {kind}"));
        let captured = capture(&syntax).unwrap();
        assert_eq!(captured.scalar_type(), ScalarType::Currency);
        assert_eq!(captured.required(), None);
        assert_eq!(captured.allow_zero_length(), None);
        assert!(captured.default_literal().is_none());
        assert!(captured.description().is_none());
        assert!(captured.primary_key_name().is_none());
        assert!(!captured.incrementing_autonumber());
        assert!(!captured.unicode_compression());
    }
}

#[test]
fn annotation_and_other_type_refusals_do_not_substitute_gallery_defaults() {
    for suffix in [
        "CURRENCY NOT NULL",
        "CURRENCY DEFAULT 0",
        "CURRENCY CONSTRAINT AmountKey PRIMARY KEY",
        "COUNTER",
        "BINARY(16)",
        "TEXT",
    ] {
        let syntax = column(&format!("ALTER TABLE Table1 ADD COLUMN Amount {suffix}"));
        assert!(matches!(capture(&syntax), Err(Error::SchemaSqlUnsupported)));
    }
}

#[test]
fn add_compression_is_refused_by_the_actual_sql_parser_profile() {
    assert!(
        parse_query(
            "ALTER TABLE Table1 ADD COLUMN Amount CURRENCY WITH COMPRESSION",
            ParseLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn unicode_identifier_spelling_reaches_native_admission_without_transliteration() {
    let syntax = column("ALTER TABLE Table1 ADD COLUMN [Beträg] CURRENCY");
    let captured = capture(&syntax).unwrap();
    assert_eq!(captured.name(), "Beträg");
}
