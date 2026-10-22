#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn native_derived_records_keep_hidden_keys_during_join_deduplication() {
    for code in [1, 2, 5, 6] {
        let mut bytes = fixture::with_rule("Caption", "derived record fixture");
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
        for (projection, expected) in [
            (
                "Id,Score",
                vec![Value::Int32(10), Value::Int32(20), Value::Int32(30)],
            ),
            (
                "Id,Score+1 AS Score",
                vec![Value::Int64(11), Value::Int64(21), Value::Int64(31)],
            ),
        ] {
            let sql = format!(
                "SELECT DISTINCTROW D.Score FROM (SELECT {projection} FROM Items) D INNER JOIN Items B ON D.Id Mod 2=B.Id Mod 2 ORDER BY D.Score"
            );
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let result = prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .unwrap();
            assert_eq!(
                result.rows(),
                expected
                    .into_iter()
                    .map(|value| vec![value])
                    .collect::<Vec<_>>()
            );
        }
    }
}
