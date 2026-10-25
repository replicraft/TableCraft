use super::support::{Host, run};
use tablecraft_core::values::{Double, Value};
use tablecraft_query::{ExecutionOptions, execute_with_schema_discovery, prepare};
use tablecraft_sql::parse_query;

#[test]
fn preparation_is_metadata_only_and_all_direct_join_leaves_are_external() {
    let host = Host::new(false);
    let sql = "SELECT A.Id,B.Id FROM (T A LEFT JOIN U B ON A.Id=B.Id) IN 'remote' ORDER BY A.Id";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();

    assert_eq!(host.remote.projections.get(), 0);
    assert_eq!(host.remote.scans.get(), 0);
    assert_eq!(host.remote.named_calls.get(), 0);
    host.allow_rows();

    let result = plan
        .execute(&host, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(2), Value::Null]
        ]
    );
    assert_eq!(host.local.scans.get(), 0);
    assert!(host.remote.projections.get() > 0);
}

#[test]
fn nested_sources_choose_their_own_namespace_and_captures_keep_outer_values() {
    let host = Host::new(false);
    let rows = run(&host, "SELECT E.Id,(SELECT Min(Id) FROM T) AS L,(SELECT Min(Id) FROM T IN 'second') AS S FROM T E IN 'remote' ORDER BY E.Id").unwrap();
    assert_eq!(
        rows,
        vec![
            vec![Value::Int32(1), Value::Int32(10), Value::Int32(100)],
            vec![Value::Int32(2), Value::Int32(10), Value::Int32(100)]
        ]
    );

    let rows = run(&host, "SELECT E.Id,(SELECT R.Id FROM T R WHERE R.Id=E.Id+9) AS L FROM T E IN 'remote' ORDER BY E.Id").unwrap();
    assert_eq!(
        rows,
        vec![
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Null]
        ]
    );
}

#[test]
fn root_named_definitions_and_domains_can_explicitly_use_external_sources() {
    let mut host = Host::new(false);
    host.definition("RemoteRows", "SELECT Id FROM T IN 'remote'");
    assert_eq!(
        run(&host, "SELECT Id FROM RemoteRows ORDER BY Id").unwrap(),
        vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    assert_eq!(
        run(&host, "SELECT DCount('*','RemoteRows')").unwrap(),
        vec![vec![Value::Int64(2)]]
    );

    let syntax = parse_query("SELECT Id FROM RemoteRows IN 'remote'", Default::default()).unwrap();
    assert!(prepare(syntax.query(), &host).is_err());
    assert_eq!(host.remote.named_calls.get(), 0);
}

#[test]
fn target_projection_hints_keep_original_filter_and_target_metadata() {
    let host = Host::new(false);
    let rows = run(&host, "SELECT Id FROM T IN 'remote' WHERE Id>1").unwrap();
    assert_eq!(rows, vec![vec![Value::Int32(2)]]);
    assert!(host.remote.projections.get() > 0);
    assert!(host.remote.saw_constraint.get());
    assert!(host.remote.hint_calls.get() > 0);
    assert_eq!(host.local.scans.get(), 0);
}

#[test]
fn collections_and_joined_presence_use_the_target_callbacks() {
    let host = Host::new(true);
    let rows = run(
        &host,
        "SELECT Id,Files.Name,Files.Size FROM T IN 'remote' ORDER BY Id",
    )
    .unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows[0],
        vec![Value::Int32(1), Value::Text("n".into()), Value::Int32(10)]
    );
    assert_eq!(
        rows[1],
        vec![Value::Int32(1), Value::Text("m".into()), Value::Int32(20)]
    );

    assert_eq!(
        run(
            &host,
            "SELECT Id FROM T IN 'remote' WHERE Files IS NOT NULL ORDER BY Id"
        )
        .unwrap(),
        vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    assert_eq!(run(&host, "SELECT A.Id FROM (T A INNER JOIN U B ON A.Id=B.Id) IN 'remote' WHERE A.Files IS NOT NULL").unwrap(), vec![vec![Value::Int32(1)]]);
    assert!(host.remote.collection_scans.get() > 0);
}

#[test]
fn named_dynamic_headings_discover_external_rows_once() {
    let mut host = Host::new(false);
    host.definition(
        "Pivoted",
        "TRANSFORM Count(*) SELECT 1 AS G FROM T IN 'remote' GROUP BY 1 PIVOT Id",
    );
    let syntax = parse_query("SELECT [1],[2] FROM Pivoted", Default::default()).unwrap();
    assert!(prepare(syntax.query(), &host).is_err());
    assert_eq!(host.remote.projections.get(), 0);

    host.allow_rows();
    let result =
        execute_with_schema_discovery(syntax.query(), &host, &[], ExecutionOptions::default())
            .unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(host.remote.projections.get(), 1);
    assert_eq!(host.remote.named_calls.get(), 0);
}

#[test]
fn external_field_policy_survives_named_and_derived_boundaries() {
    use tablecraft_query::source::{TextCollationId, TextPolicy};

    let mut host = Host::new(false);
    host.definition("RemoteNames", "SELECT Label FROM T IN 'remote'");
    for sql in [
        "SELECT Label FROM T IN 'remote'",
        "SELECT Label FROM RemoteNames",
        "SELECT Label FROM (SELECT Label FROM T IN 'remote') X",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &host).unwrap();
        assert_eq!(
            plan.output_text_policies(),
            Some([TextPolicy::Column(TextCollationId::from_host(2))].as_slice())
        );
        assert_eq!(host.remote.projections.get(), 0);
    }
}

#[test]
fn external_named_and_derived_unions_keep_runtime_arithmetic_profiles() {
    use tablecraft_query::{OutputType, PreparationOptions, prepare_with_options};
    use tablecraft_schema::ScalarType;

    let mut host = Host::new(false);
    host.definition(
        "RemoteRatios",
        "SELECT CDec(Id)/CDbl(2) AS V FROM T IN 'remote'",
    );
    for sql in [
        "SELECT V FROM (SELECT CDec(Id)/CDbl(2) AS V FROM T IN 'remote') X UNION ALL SELECT CLng(2)",
        "SELECT V FROM RemoteRatios UNION ALL SELECT CLng(2)",
        "SELECT V FROM (SELECT V FROM RemoteRatios UNION ALL SELECT CLng(2)) X",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        for prepared_declared in [false, true] {
            let plan = prepare_with_options(
                syntax.query(),
                &host,
                PreparationOptions {
                    declared_arithmetic: prepared_declared,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(plan.output_types(), Some([OutputType::Dynamic].as_slice()));
            host.allow_rows();

            for declared in [false, true] {
                let options = ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                };
                let result = plan.execute(&host, &[], options).unwrap();
                assert_eq!(result.rows().len(), 3);
                for row in result.rows() {
                    assert!(
                        if declared {
                            matches!(row[0], Value::Double(_))
                        } else {
                            matches!(row[0], Value::Decimal(_))
                        },
                        "{sql}"
                    );
                }

                let OutputType::Scalar { data_type, .. } = result.output_types()[0] else {
                    panic!("active arithmetic profile must have a scalar descriptor");
                };
                assert_eq!(
                    data_type.scalar_type(),
                    if declared {
                        ScalarType::Double
                    } else {
                        ScalarType::Decimal
                    },
                );
                if declared {
                    assert_eq!(
                        result.rows(),
                        &[
                            vec![Value::Double(Double::from_float(0.5))],
                            vec![Value::Double(Double::from_float(1.0))],
                            vec![Value::Double(Double::from_float(2.0))]
                        ]
                    );
                } else {
                    for (row, coefficient) in result.rows().iter().zip([5, 10, 20]) {
                        let Value::Decimal(value) = row[0] else {
                            panic!("materialized profile must preserve Decimal carriers");
                        };
                        assert_eq!(value.rescale_exact(1).unwrap().coefficient(), coefficient);
                    }
                }
            }
        }
    }
}
