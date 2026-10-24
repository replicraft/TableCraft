use super::queries::query_round_trip;
use super::*;

#[test]
fn access_names_do_not_inherit_other_dialects_keywords() {
    for sql in [
        "SELECT ID, Ref, Type FROM Records AS R WHERE R.ID = 1",
        "CREATE TABLE Records (ID COUNTER PRIMARY KEY, Ref LONG)",
        "CREATE VIEW RecordView (ID) AS SELECT ID FROM Records",
        "SELECT Code AS ID FROM Samples",
        "SELECT Code FROM Samples ID WHERE ID.Code = 1",
        "SELECT ID FROM Samples IN ID.accdb",
        "SELECT StDev(Code), StDevP(Code), Var(Code), VarP(Code) FROM Samples",
    ] {
        query_round_trip(sql);
    }
}

#[test]
fn access_reserved_words_require_brackets_in_name_positions() {
    for name in [
        "Level",
        "TableID",
        "Mod",
        "Eqv",
        "Imp",
        "Compression",
        "OwnerAccess",
    ] {
        let sql = format!("SELECT {name} FROM Samples");
        assert!(
            crate::parse_query(&sql, ParseLimits::default()).is_err(),
            "accepted {sql}"
        );
        query_round_trip(&format!("SELECT [{name}] FROM Samples"));
        assert!(
            crate::parse_query(
                &format!("SELECT Code AS {name} FROM Samples"),
                ParseLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn value_is_a_parameter_type_without_becoming_a_stored_column_type() {
    for sql in [
        "PARAMETERS P VALUE; SELECT P",
        "PARAMETERS [Prompt?] vAlUe, N LONG; SELECT [Prompt?], N",
        "CREATE PROCEDURE ReadValue P VALUE AS SELECT P",
    ] {
        query_round_trip(sql);
    }

    for sql in [
        "PARAMETERS P VALUE(2); SELECT P",
        "PARAMETERS P [VALUE]; SELECT P",
        "PARAMETERS P VARIANT; SELECT P",
        "CREATE TABLE T (N VALUE)",
    ] {
        assert!(
            crate::parse_query(sql, ParseLimits::default()).is_err(),
            "{sql}"
        );
    }
}

#[test]
fn dot_members_keep_keyword_spellings_without_relaxing_other_name_positions() {
    for sql in [
        "SELECT Tags.Value FROM Items",
        "SELECT T.Tags.vAlUe FROM Items AS T",
        "SELECT T.[Value] FROM Items AS T",
        "SELECT msysobjects.Connect FROM msysobjects",
        "SELECT T.Connect, T.SELECT, T.From, T.Order FROM Items AS T",
        "SELECT T.Tags.Connect FROM Items AS T WHERE T.Connect IS NULL",
        "SELECT T.Null, T.True, T.False, T.Date FROM Items AS T",
        "SELECT T.From FROM Items AS T WHERE T.Where=1 GROUP BY T.Group HAVING Count(*)>0 ORDER BY T.Order",
    ] {
        let parsed = crate::parse_query(sql, crate::ParseLimits::default()).unwrap();
        let formatted = parsed.query().to_string();
        assert_eq!(
            crate::parse_query(&formatted, crate::ParseLimits::default())
                .unwrap()
                .query(),
            parsed.query()
        );
    }
    for sql in [
        "SELECT Value FROM Items",
        "SELECT Tags!Value FROM Items",
        "SELECT Connect FROM Items",
        "SELECT Tags!Connect FROM Items",
        "SELECT Code AS Connect FROM Items",
        "SELECT Code FROM Connect",
        "SELECT T.Connect() FROM Items AS T",
        "SELECT T. FROM Items AS T",
        "SELECT T.; SELECT 1 FROM Items AS T",
    ] {
        assert!(
            crate::parse_query(sql, crate::ParseLimits::default()).is_err(),
            "{sql}"
        );
    }
}
