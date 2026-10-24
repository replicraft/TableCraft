use std::cell::Cell;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_query::{OutputType, QueryError, Source, prepare, source::MemorySource};
use tablecraft_schema::{Column, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

struct Views {
    definitions: Vec<(String, Query)>,
    scans: Cell<usize>,
}

impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .definitions
            .iter()
            .find_map(|(stored, query)| stored.eq_ignore_ascii_case(name).then_some(query)))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::UnknownName)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        Err(QueryError::Source)
    }
}

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn value(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}

#[test]
fn explicit_decimal_parameters_retain_exact_descriptors_and_representations() {
    let source = MemorySource::new(Vec::new()).unwrap();
    for name in ["DECIMAL", "NUMERIC"] {
        for precision in 1..=28 {
            for scale in 0..=precision {
                let query = syntax(&format!(
                    "PARAMETERS P {name}({precision},{scale}); SELECT P AS Amount"
                ));
                let plan = prepare(&query, &source).unwrap();
                assert_eq!(
                    plan.output_types().unwrap(),
                    [OutputType::Scalar {
                        data_type: TypeDescriptor::decimal(precision, scale).unwrap(),
                        nullable: true,
                    }]
                );
                let limit = 10_i128.pow(u32::from(precision));
                for coefficient in [0, limit - 1, -limit + 1] {
                    let supplied = value(coefficient, scale);
                    let result = plan
                        .execute(&source, &[("p", supplied.clone())], Default::default())
                        .unwrap();
                    assert_eq!(result.rows(), [vec![supplied]]);
                }
                for invalid in [value(limit, scale), value(-limit, scale), Value::Int64(0)] {
                    assert_eq!(
                        plan.execute(&source, &[("P", invalid)], Default::default())
                            .err(),
                        Some(QueryError::Parameter)
                    );
                }
                let null = plan
                    .execute(&source, &[("P", Value::Null)], Default::default())
                    .unwrap();
                assert_eq!(null.rows(), [vec![Value::Null]]);
            }
        }
    }
}

#[test]
fn constrained_parameters_do_not_round_rescale_or_restrict_bare_decimal() {
    let source = MemorySource::new(Vec::new()).unwrap();
    let query = syntax("PARAMETERS P DECIMAL(4,2); SELECT P");
    let plan = prepare(&query, &source).unwrap();
    for invalid in [value(1, 0), value(10, 1), value(1000, 3)] {
        assert_eq!(
            plan.execute(&source, &[("P", invalid)], Default::default())
                .err(),
            Some(QueryError::Parameter)
        );
    }
    for spelling in ["DECIMAL", "NUMERIC"] {
        let query = syntax(&format!("PARAMETERS P {spelling}; SELECT P"));
        let plan = prepare(&query, &source).unwrap();
        for supplied in [
            value(1000, 3),
            value(79_228_162_514_264_337_593_543_950_335, 0),
        ] {
            assert_eq!(
                plan.execute(&source, &[("P", supplied.clone())], Default::default())
                    .unwrap()
                    .rows(),
                [vec![supplied]]
            );
        }
    }
}

#[test]
fn named_scopes_merge_only_matching_constraints_and_keep_expression_scopes() {
    let source = Views {
        definitions: [
            ("A", "PARAMETERS P DECIMAL(4,2); SELECT P AS Amount"),
            ("B", "PARAMETERS p NUMERIC(4,2); SELECT p AS Amount"),
            (
                "WrongScale",
                "PARAMETERS P DECIMAL(4,1); SELECT P AS Amount",
            ),
            (
                "WrongPrecision",
                "PARAMETERS P DECIMAL(5,2); SELECT P AS Amount",
            ),
            ("Bare", "PARAMETERS P DECIMAL; SELECT P AS Amount"),
            ("Unbound", "SELECT P AS Amount"),
        ]
        .into_iter()
        .map(|(name, query)| (name.into(), syntax(query)))
        .collect(),
        scans: Cell::new(0),
    };
    for sql in [
        "SELECT A.Amount,B.Amount FROM A,B",
        "SELECT Amount FROM A UNION ALL SELECT Amount FROM B",
        "PARAMETERS P DECIMAL(4,2); SELECT (SELECT Amount FROM A),P",
    ] {
        let query = syntax(sql);
        let plan = prepare(&query, &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        let result = plan
            .execute(&source, &[("P", value(123, 2))], Default::default())
            .unwrap();
        assert!(
            result
                .rows()
                .iter()
                .flatten()
                .all(|cell| cell == &value(123, 2))
        );
    }
    for name in ["WrongScale", "WrongPrecision", "Bare"] {
        let query = syntax(&format!("SELECT A.Amount FROM A,{name}"));
        assert_eq!(prepare(&query, &source).err(), Some(QueryError::Parameter));
    }
    let query = syntax("PARAMETERS P DECIMAL(4,2); SELECT * FROM Unbound");
    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::UnknownName)
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn invalid_decimal_constraints_fail_during_metadata_preparation() {
    let source = Views {
        definitions: Vec::new(),
        scans: Cell::new(0),
    };
    for declaration in [
        "DECIMAL(0,0)",
        "DECIMAL(29,0)",
        "DECIMAL(4,5)",
        "NUMERIC(256,0)",
        "NUMERIC(4,256)",
        "DECIMAL(4294967295,0)",
        "DECIMAL(4)",
        "LONG(4,2)",
    ] {
        let query = syntax(&format!("PARAMETERS P {declaration}; SELECT P"));
        assert_eq!(
            prepare(&query, &source).err(),
            Some(QueryError::Unsupported),
            "{declaration}"
        );
    }
    assert_eq!(source.scans.get(), 0);
}
