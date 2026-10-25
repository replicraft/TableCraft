use super::support::{Host, options, run};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    OutputType, QueryError, prepare,
    source::{TextCollationId, TextPolicy},
};
use tablecraft_sql::parse_query;

#[test]
fn same_named_definitions_in_three_namespaces_keep_local_dependencies_and_aliases() {
    let host = Host::new();
    let sql = "SELECT R.Id,A.Id,B.Id FROM Same R,(SELECT Id FROM Nested IN 'one') A,(SELECT Id FROM Nested IN 'two') B";
    assert_eq!(
        run(&host, sql).unwrap().rows(),
        &[vec![Value::Int32(10), Value::Int32(1), Value::Int32(2)]]
    );
}

#[test]
fn named_owner_applies_to_nested_selects_domains_and_written_eval_arguments() {
    let mut host = Host::new();
    let owned = concat!(
        "SELECT Id,(SELECT Min(Id) FROM T) AS N,",
        "DSum('Id','T') AS D,DSum('Id','Same') AS Named,",
        "Eval(Formula) AS E,",
        "(SELECT I.Id FROM T I WHERE I.Id=O.Id) AS Captured FROM T O",
    );
    host.one.define("Owned", owned);

    let result = run(&host, "SELECT * FROM Owned IN 'one'").unwrap();
    assert_eq!(
        result.rows()[0][..4],
        [
            Value::Int32(1),
            Value::Int32(1),
            Value::Int64(1),
            Value::Int64(1)
        ]
    );
    assert_eq!(result.rows()[0][4], Value::Int64(101));
    assert_eq!(result.rows()[0][5], Value::Int32(1));

    let direct = run(
        &host,
        "SELECT (SELECT Min(Id) FROM T) AS N,Id FROM T IN 'one'",
    )
    .unwrap();
    assert_eq!(direct.rows(), &[vec![Value::Int32(10), Value::Int32(1)]]);

    host.one.define("BadEval", "SELECT Eval('Id') FROM T");
    assert_eq!(
        run(&host, "SELECT * FROM BadEval IN 'one'").err(),
        Some(EvalError::UnboundField.into())
    );
    host.one
        .define("DomainEval", "SELECT Eval('DSum(''Id'',''T'')') FROM T");
    assert_eq!(
        run(&host, "SELECT * FROM DomainEval IN 'one'").err(),
        Some(EvalError::UnsupportedExpression.into())
    );
}

#[test]
fn explicit_in_inside_an_external_definition_uses_the_operation_root_registry() {
    let mut host = Host::new();
    let redirect = concat!(
        "SELECT A.Id,B.Id AS Other,(SELECT Min(Id) FROM T) AS Local ",
        "FROM T A,(SELECT Id FROM T IN 'two') B",
    );
    host.one.define("Redirect", redirect);

    assert_eq!(
        run(&host, "SELECT * FROM Redirect IN 'one'")
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1), Value::Int32(2), Value::Int32(1)]]
    );
    host.one.define("Missing", "SELECT Id FROM T IN 'unmapped'");
    let scans = host.scans();
    assert!(run(&host, "SELECT * FROM Missing IN 'one'").is_err());
    assert_eq!(host.scans(), scans);
}

#[test]
fn parameter_scope_and_named_cycles_are_isolated_by_namespace() {
    let mut host = Host::new();
    host.one
        .define("Param", "PARAMETERS P LONG; SELECT Id+P AS V FROM T");
    let syntax = parse_query("SELECT V FROM Param IN 'one'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    assert_eq!(host.scans(), 0);
    assert_eq!(
        plan.execute(&host, &[("P", Value::Int32(5))], options())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(6)]]
    );

    host.one.define("Undeclared", "SELECT P FROM T");
    assert_eq!(
        run(
            &host,
            "PARAMETERS P LONG; SELECT * FROM Undeclared IN 'one'"
        )
        .err(),
        Some(QueryError::UnknownName)
    );

    host.one.define("Cycle", "SELECT * FROM Cycle");
    assert_eq!(
        run(&host, "SELECT * FROM Cycle IN 'one'").err(),
        Some(QueryError::DependencyCycle)
    );
}

#[test]
fn target_policies_and_paired_arithmetic_types_survive_named_unions() {
    let mut host = Host::new();
    let syntax = parse_query("SELECT Label FROM Nested IN 'one'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    assert_eq!(
        plan.output_text_policies(),
        Some([TextPolicy::Column(TextCollationId::from_host(11))].as_slice())
    );
    host.one
        .define("Ratio", "SELECT CDec(Id)/CDbl(2) AS V FROM T");
    let syntax = parse_query(
        "SELECT V FROM Ratio IN 'one' UNION ALL SELECT CLng(2)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    assert_eq!(plan.output_types(), Some([OutputType::Dynamic].as_slice()));
    for declared in [false, true] {
        let result = plan
            .execute(
                &host,
                &[],
                tablecraft_query::ExecutionOptions {
                    declared_integral: declared,
                    ..options()
                },
            )
            .unwrap();
        assert_eq!(result.rows().len(), 2);
        assert!(result.rows().iter().all(|row| if declared {
            matches!(row[0], Value::Double(_))
        } else {
            matches!(row[0], Value::Decimal(_))
        }));
    }
}
