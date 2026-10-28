#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn native_subquery_projections_capture_record_owners_in_the_correct_frame() {
    for code in [1, 2, 5, 6] {
        let mut bytes = fixture::with_rule("Caption", "DISTINCTROW subquery fixture");
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
        .unwrap();
        for (sql, expected) in [
            (
                "SELECT DISTINCTROW 1 AS V FROM Items A INNER JOIN Items B ON A.Id Mod 2=B.Id Mod 2",
                vec![vec![Value::Int64(1)]; 5],
            ),
            (
                "SELECT DISTINCTROW Count(*) AS N FROM Items A INNER JOIN Items B ON A.Id Mod 2=B.Id Mod 2 GROUP BY A.Id Mod 2 ORDER BY N",
                vec![vec![Value::Int64(1)], vec![Value::Int64(4)]],
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
        for (expression, expected) in [
            ("(SELECT A.Score)", vec![10, 20, 30]),
            ("(SELECT (SELECT A.Score))", vec![10, 20, 30]),
            ("(SELECT A.Score+B.Score)", vec![20, 40, 40, 40, 60]),
        ] {
            let sql = format!(
                "SELECT DISTINCTROW {expression} AS V FROM Items A INNER JOIN Items B ON A.Id Mod 2=B.Id Mod 2 ORDER BY V"
            );
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let result = prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            let expected = expected
                .into_iter()
                .map(|value| vec![Value::Int32(value)])
                .collect::<Vec<_>>();
            assert_eq!(result.rows(), expected);
        }
    }
}
