use super::*;
use tablecraft_core::values::Guid;

impl Restricted {
    fn guids(ignore: bool) -> Self {
        Self {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Guid), ("Y", ScalarType::Int32)],
                [
                    None,
                    Some([0; 16]),
                    Some([0x80; 16]),
                    Some([0x80; 16]),
                    Some([0xff; 16]),
                ]
                .into_iter()
                .map(|value| {
                    vec![
                        value.map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
                        Value::Int32(1),
                    ]
                })
                .collect(),
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        }
    }
}

#[test]
fn guid_points_keep_nulls_duplicates_reversed_operands_and_residuals() {
    for (filter, expected) in [
        (
            "X={GUID {80808080-8080-8080-8080-808080808080}}",
            ScanConstraint::Guid {
                column: 0,
                value: Guid::from_bytes([0x80; 16]),
            },
        ),
        (
            "{GUID {00000000-0000-0000-0000-000000000000}}=X",
            ScanConstraint::Guid {
                column: 0,
                value: Guid::from_bytes([0; 16]),
            },
        ),
        (
            "X={GUID {80808080-8080-8080-8080-808080808080}} AND X={GUID {80808080-8080-8080-8080-808080808080}}",
            ScanConstraint::Guid {
                column: 0,
                value: Guid::from_bytes([0x80; 16]),
            },
        ),
        ("X IS NULL", ScanConstraint::Null { column: 0 }),
    ] {
        let source = Restricted::guids(false);
        let ignoring = Restricted::guids(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            run(&sql, &ignoring).unwrap().rows()
        );
        assert_eq!(source.seen.get(), Some(expected));
        assert_eq!(source.scans.get(), 0);
    }
    for filter in [
        "X IS NOT NULL",
        "X=NULL",
        "X={GUID {00000000-0000-0000-0000-000000000000}} AND 1/0=0",
        "X={GUID {00000000-0000-0000-0000-000000000000}} AND Y=1",
        "X={GUID {00000000-0000-0000-0000-000000000000}} OR X IS NULL",
        "X>{GUID {00000000-0000-0000-0000-000000000000}}",
        "X=GUIDFromString('00000000-0000-0000-0000-000000000000')",
        "X={GUID {00000000-0000-0000-0000-000000000000}} AND X IS NULL",
        "X={GUID {00000000-0000-0000-0000-000000000000}} AND X={GUID {FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}}",
    ] {
        let source = Restricted::guids(false);
        let ignoring = Restricted::guids(true);
        let sql = format!("SELECT X FROM T WHERE {filter}");
        assert_eq!(
            run(&sql, &source).map(|result| result.rows().to_vec()),
            run(&sql, &ignoring).map(|result| result.rows().to_vec()),
            "{filter}"
        );
        assert_eq!(source.seen.get(), None, "{filter}");
        assert_eq!(source.scans.get(), 1);
    }
}

#[test]
fn guid_parameters_rebind_without_coercion_or_stale_points() {
    let source = Restricted::guids(false);
    let ignoring = Restricted::guids(true);
    let parsed = parse_query(
        "PARAMETERS p GUID; SELECT X FROM T WHERE p=X",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    for value in [
        Value::Guid(Guid::from_bytes([0; 16])),
        Value::Guid(Guid::from_bytes([0x80; 16])),
        Value::Guid(Guid::from_bytes([1; 16])),
        Value::Null,
    ] {
        source.seen.set(None);
        let parameters = [("p", value.clone())];
        assert_eq!(
            plan.execute(&source, &parameters, ExecutionOptions::default())
                .map(|result| result.rows().to_vec()),
            plan.execute(&ignoring, &parameters, ExecutionOptions::default())
                .map(|result| result.rows().to_vec())
        );
        assert_eq!(source.seen.get().is_some(), value != Value::Null);
    }
    source.seen.set(None);
    assert!(
        plan.execute(
            &source,
            &[(
                "p",
                Value::Text("00000000-0000-0000-0000-000000000000".into())
            )],
            ExecutionOptions::default()
        )
        .is_err()
    );
    assert_eq!(source.seen.get(), None);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4755494448494e54),
        ..Default::default()
    })]
    #[test]
    fn every_guid_bit_survives_literal_and_parameter_point_extraction(
        bytes in proptest::prelude::any::<[u8; 16]>(),
        other in proptest::prelude::any::<[u8; 16]>(),
    ) {
        let value = Guid::from_bytes(bytes);
        let rows = vec![vec![Value::Guid(value)], vec![Value::Null], vec![Value::Guid(Guid::from_bytes(other))]];
        let source = Restricted {
            source: MemorySource::new(vec![table("T", &[("X", ScalarType::Guid)], rows.clone())]).unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore: false,
        };
        let hex = bytes.iter().map(|byte| format!("{byte:02X}")).collect::<String>();
        let literal = format!("{{GUID {{{}-{}-{}-{}-{}}}}}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]);
        let expected = rows.into_iter().filter(|row| row[0] == Value::Guid(value)).collect::<Vec<_>>();
        let sql = format!("SELECT X FROM T WHERE X={literal}");
        let result = run(&sql, &source).unwrap();
        proptest::prop_assert_eq!(result.rows(), &expected);
        proptest::prop_assert_eq!(source.seen.get(), Some(ScanConstraint::Guid { column: 0, value }));
        let parsed = parse_query("PARAMETERS p GUID; SELECT X FROM T WHERE p=X", Default::default()).unwrap();
        let result = prepare(parsed.query(), &source).unwrap().execute(&source, &[("p", Value::Guid(value))], ExecutionOptions::default()).unwrap();
        proptest::prop_assert_eq!(result.rows(), expected);
        proptest::prop_assert_eq!(source.scans.get(), 0);
    }
}
