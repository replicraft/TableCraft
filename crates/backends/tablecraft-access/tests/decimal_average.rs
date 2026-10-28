#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/decimal_indexes/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Decimal, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, OutputType, prepare};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{ParseLimits, parse_query};

struct Reads {
    bytes: Cursor<Vec<u8>>,
    data: Rc<Cell<usize>>,
}

impl tablecraft_ace::NativeReadSource for Reads {}

impl Read for Reads {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        // Independent fixture geometry: user records occupy page 12.
        if self.bytes.position() == 12 * 4096 {
            self.data.set(self.data.get() + 1);
        }
        self.bytes.read(output)
    }
}

impl Seek for Reads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn native_decimal_means_widen_totals_without_scanning_during_binding() {
    // Native NUMERIC has at most 28 declared digits, below the core carrier's
    // 29-digit top band. Nine maximum fields exceed even the core 96-bit sum.
    let maximum = 9_999_999_999_999_999_999_999_999_999_i128;
    for code in [1, 2, 5, 6] {
        for scale in [0, 28] {
            for sign in [-1, 1] {
                let values = [Some(sign * maximum); 9];
                let mut nullable = values.to_vec();
                nullable.insert(4, None);
                let count = Rc::new(Cell::new(0));
                let source = NativeSource::new(
                    Reads {
                        bytes: Cursor::new(fixture::database_for(
                            code, false, 28, scale, &nullable, 0,
                        )),
                        data: count.clone(),
                    },
                    CatalogScope::new(7),
                    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
                    &|| false,
                )
                .unwrap();
                let parsed = parse_query(
                    "SELECT Avg(Score), DAvg('Score','Items') FROM Items",
                    ParseLimits::default(),
                )
                .unwrap();
                let plan = prepare(parsed.query(), &source).unwrap();
                assert_eq!(count.get(), 0, "binding must only read metadata");
                let OutputType::Scalar {
                    data_type,
                    nullable,
                } = &plan.output_types().unwrap()[0]
                else {
                    panic!("declared Decimal mean lost its result type");
                };
                assert_eq!(data_type.scalar_type(), ScalarType::Decimal);
                assert!(*nullable);
                let result = plan
                    .execute(&source, &[], ExecutionOptions::default())
                    .unwrap();
                let expected = Value::Decimal(Decimal::new(sign * maximum, scale).unwrap());
                assert_eq!(result.rows(), [vec![expected.clone(), expected]]);
                assert!(count.get() > 0);
            }
        }
    }
}
