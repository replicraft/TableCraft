#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn native_grouped_queries_run_key_correlations_and_reject_ungrouped_captures() {
    for code in [1, 2, 5, 6] {
        let mut bytes = fixture::with_rule("Caption", "grouped query fixture");
        if code != 1 {
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            bytes[20] = code;
        }
        let source = NativeSource::new(
            Cursor::new(bytes),
            CatalogScope::new(7),
            ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
            &|| false,
        )
        .unwrap_or_else(|error| panic!("profile {code}: {error}"));
        let parsed = parse_query(
            "SELECT O.Id Mod 2 AS K,Sum(O.Score),(SELECT Count(*) FROM Items I WHERE I.Id Mod 2=O.Id Mod 2) FROM Items O GROUP BY O.Id Mod 2 HAVING EXISTS(SELECT I.Id FROM Items I WHERE I.Id Mod 2=O.Id Mod 2) ORDER BY K",
            ParseLimits::default(),
        ).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let result = plan
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int64(0), Value::Int64(20), Value::Int64(1)],
                vec![Value::Int64(1), Value::Int64(40), Value::Int64(2)]
            ]
        );
        for (sql, expected) in [
            (
                "SELECT DISTINCT O.Id Mod 2 FROM Items O ORDER BY O.Id Mod 2",
                vec![vec![Value::Int64(0)], vec![Value::Int64(1)]],
            ),
            (
                "SELECT DISTINCT Count(*) FROM Items GROUP BY Id Mod 2 ORDER BY Count(*)",
                vec![vec![Value::Int64(1)], vec![Value::Int64(2)]],
            ),
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                expected
            );
        }
        let bad = parse_query(
            "SELECT (SELECT O.Score) FROM Items O GROUP BY O.Id Mod 2",
            ParseLimits::default(),
        )
        .unwrap();
        assert_eq!(
            prepare(bad.query(), &source).err(),
            Some(QueryError::Grouping)
        );
    }
}
