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
