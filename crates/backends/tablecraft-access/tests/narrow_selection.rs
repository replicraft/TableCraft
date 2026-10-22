#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "../../../engines/tablecraft-ace/tests/narrow_indexes/fixture.rs"]
mod narrow_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16/scalar_fixture.rs"]
mod scalar_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, prepare};
use tablecraft_sql::parse_query;

#[test]
fn narrow_seeks_reuse_small_retention_limits_without_cache_contamination() {
    for byte in [false, true] {
        for code in [1, 2, 5, 6] {
            for descending in [false, true] {
                let values = if byte {
                    [Some(0), Some(128), Some(255)]
                } else {
                    [Some(-32768), Some(0), Some(32767)]
                };
                let mut bytes = narrow_fixture::database(code, !byte, descending, &values, 0);
                if code == 1 {
                    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
                }
                let source = NativeSource::new_with_snapshot_limits(
                    Cursor::new(bytes),
                    CatalogScope::new(1),
                    ResourceBudget::new(32 * 1024 * 1024, 16384, 1_000_000),
                    &|| false,
                    TableSnapshotLimits::new(1, 1).unwrap(),
                )
                .unwrap()
                .with_index("Items", "BigKey")
                .unwrap();
                for (slot, value) in values.into_iter().enumerate() {
                    let sql = format!("SELECT Id FROM Items WHERE Score={}", value.unwrap());
                    let query = parse_query(&sql, Default::default()).unwrap();
                    let result = prepare(query.query(), &source)
                        .unwrap()
                        .execute(&source, &[], Default::default())
                        .unwrap();
                    assert_eq!(result.rows(), [vec![Value::Int32(slot as i32 + 1)]]);
                }
                let query = parse_query("SELECT Count(*) FROM Items", Default::default()).unwrap();
                assert_eq!(
                    prepare(query.query(), &source)
                        .unwrap()
                        .execute(&source, &[], Default::default())
                        .err(),
                    Some(QueryError::StructureLimit)
                );
            }
        }
    }
}
