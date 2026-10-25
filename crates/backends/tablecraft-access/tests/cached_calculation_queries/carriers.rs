use super::{Watching, budget, execute, scalar_fixture};
use std::{cell::Cell, io::Cursor, rc::Rc};
use tablecraft_access::NativeSource;
use tablecraft_core::values::{Guid, Single, Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn new_cached_carriers_bind_without_records_and_keep_exact_query_values() {
    let cases = [
        (2, vec![255], Value::Byte(255)),
        (3, (-123_i16).to_le_bytes().to_vec(), Value::Int16(-123)),
        (19, i64::MAX.to_le_bytes().to_vec(), Value::Int64(i64::MAX)),
        (
            6,
            0x80000000_u32.to_le_bytes().to_vec(),
            Value::Single(Single::from_bits(0x80000000)),
        ),
        (15, vec![0; 16], Value::Guid(Guid::from_bytes([0; 16]))),
    ];
    for code in [5, 6] {
        for (kind, payload, expected) in &cases {
            let bytes =
                scalar_fixture::database(code, *kind, 39, false, &[Some(payload.clone()), None]);
            let ordinary = NativeSource::new(
                Cursor::new(bytes.clone()),
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap();
            assert!(execute(&ordinary, "SELECT Score FROM Items").is_err());

            let forbidden = Rc::new(Cell::new(true));
            let reads = Rc::new(Cell::new(0));
            let source = NativeSource::new(
                Watching {
                    data: Cursor::new(bytes),
                    forbid: forbidden.clone(),
                    reads: reads.clone(),
                },
                CatalogScope::new(1),
                budget(),
                &|| false,
            )
            .unwrap()
            .with_stored_calculations()
            .unwrap();
            let query = parse_query("SELECT Score FROM Items", ParseLimits::default()).unwrap();
            let prepared = prepare(query.query(), &source).unwrap();
            assert_eq!(reads.get(), 0);
            assert_eq!(
                source.is_stored_calculation("Items", 1).unwrap(),
                Some(true)
            );

            forbidden.set(false);
            assert_eq!(
                prepared
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap()
                    .rows(),
                [vec![expected.clone()], vec![Value::Null]],
            );
            forbidden.set(true);
            assert_eq!(
                execute(&source, "SELECT Count(Score) FROM Items").unwrap(),
                [vec![Value::Int64(1)]]
            );
        }
    }
}
