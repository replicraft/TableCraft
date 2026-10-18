use super::queries::query_round_trip;
use super::*;
use crate::query::{Statement, definition::*};

#[test]
fn schema_query_families_round_trip_without_applying_changes() {
    for sql in [
        "CREATE TABLE Samples (Code COUNTER CONSTRAINT SampleKey PRIMARY KEY, Label TEXT(60) NOT NULL WITH COMP, Amount DECIMAL(28, 4))",
        "CREATE TEMPORARY TABLE ScratchRows (Code AUTOINCREMENT(1, 1), ParentID LONG CONSTRAINT ParentRef REFERENCES Parents (Code) ON UPDATE CASCADE ON DELETE SET NULL)",
        "CREATE TABLE Samples (Code LONG PRIMARY KEY, Label CHAR(50), CONSTRAINT LabelUnique UNIQUE (Label), CONSTRAINT RequiredFields NOT NULL (Code, Label))",
        "CREATE TABLE Samples (Code LONG, ParentID LONG, CONSTRAINT ParentRef FOREIGN KEY NO INDEX (ParentID) REFERENCES Parents ON DELETE CASCADE ON UPDATE SET NULL)",
        "CREATE TABLE Samples (Code LONG CONSTRAINT RequiredCode NOT NULL)",
        "CREATE TABLE Samples (Code LONG UNIQUE)",
        "CREATE UNIQUE INDEX Codes ON Samples (Code DESC, Label ASC) WITH DISALLOW NULL",
        "CREATE INDEX Codes ON Samples (Code) WITH IGNORE NULL",
        "CREATE INDEX Codes ON Samples (Code) WITH PRIMARY",
        "CREATE INDEX Codes ON Samples (Code)",
        "ALTER TABLE Samples ADD COLUMN Extra TEXT(50) NOT NULL CONSTRAINT ExtraUnique UNIQUE",
        "ALTER TABLE Samples ALTER COLUMN Extra TEXT(100)",
        "ALTER TABLE Samples ADD CONSTRAINT SampleKey PRIMARY KEY (Code)",
        "ALTER TABLE Samples DROP COLUMN Extra",
        "ALTER TABLE Samples DROP CONSTRAINT SampleKey",
        "DROP INDEX Codes ON Samples",
        "DROP TABLE Samples",
        "create table t (c long, constraint pk primary key (c))",
    ] {
        query_round_trip(sql);
    }
}

#[test]
fn schema_modifiers_fields_and_referential_actions_remain_distinct() {
    let parsed = query_round_trip(
        "CREATE TEMPORARY TABLE ScratchRows (Code LONG, CONSTRAINT ParentRef FOREIGN KEY NO INDEX (Code) REFERENCES Parents ([ID]) ON DELETE SET NULL ON UPDATE CASCADE)",
    );
    let Statement::Definition(definition) = &parsed.query().statement else {
        panic!("definition expected")
    };
    let Definition::CreateTable(table) = definition.as_ref() else {
        panic!("table expected")
    };
    assert!(table.temporary);
    assert_eq!(table.elements.len(), 2);
    let TableElement::Constraint(Constraint {
        kind: ConstraintKind::ForeignKey(reference),
        ..
    }) = &table.elements[1]
    else {
        panic!("foreign key expected")
    };
    assert!(reference.no_index);
    assert_eq!(reference.fields[0].as_str(), "Code");
    assert_eq!(reference.foreign_fields[0].as_str(), "ID");
    assert_eq!(reference.on_update, Some(ReferentialAction::Cascade));
    assert_eq!(reference.on_delete, Some(ReferentialAction::SetNull));
}

#[test]
fn malformed_schema_and_foreign_dialects_fail_closed() {
    for sql in [
        "CREATE TABLE Samples ()",
        "CREATE TABLE Samples (CONSTRAINT SampleKey PRIMARY KEY (Code))",
        "CREATE TABLE Samples (Code LONG,)",
        "CREATE TABLE Samples (Code UnknownType)",
        "CREATE TABLE Samples (Code LONG DEFAULT 1)",
        "CREATE TABLE Samples (Code LONG CHECK (Code > 0))",
        "CREATE TABLE Samples AS SELECT Code FROM Others",
        "CREATE TABLE IF NOT EXISTS Samples (Code LONG)",
        "CREATE TEMPORARY INDEX Codes ON Samples (Code)",
        "CREATE INDEX Codes ON Samples ()",
        "CREATE INDEX Codes ON Samples (Code + 1)",
        "CREATE INDEX Codes ON Samples (Code) WITH IGNORE NULL WITH PRIMARY",
        "ALTER TABLE Samples ADD COLUMN A LONG, ADD COLUMN B LONG",
        "ALTER TABLE Samples ADD CONSTRAINT ParentRef FOREIGN KEY () REFERENCES Parents",
        "ALTER TABLE Samples ADD CONSTRAINT ParentRef FOREIGN KEY (Code) REFERENCES Parents ON DELETE RESTRICT",
        "ALTER TABLE Samples ADD CONSTRAINT ParentRef FOREIGN KEY (Code) REFERENCES Parents ON DELETE CASCADE ON DELETE SET NULL",
        "ALTER TABLE Samples ADD CONSTRAINT ParentRef PRIMARY KEY ()",
        "DROP TABLE IF EXISTS Samples",
        "DROP TABLE Samples CASCADE",
        "DROP INDEX Codes",
        "PARAMETERS x LONG; CREATE TABLE Samples (Code LONG)",
        "SELECT * FROM (CREATE TABLE Samples (Code LONG)) AS s",
    ] {
        assert!(
            crate::parse_query(sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
    }
}
