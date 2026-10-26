//! Each additional admitted recursion shape is isolated from the test process.
use super::*;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Table,
    Reference,
    Union,
    UnionLater,
    MixedInline,
    FixedCrosstab,
    DynamicCrosstab,
}

fn shape_chain(shape: Shape) -> Definitions {
    let mut source = Definitions::chain(50);
    for (index, (_, query)) in source.queries.iter_mut().enumerate() {
        let target = if index == 49 {
            "Data".to_owned()
        } else {
            format!("Q{}", index + 1)
        };
        let sql = match shape {
            Shape::Table => format!("TABLE {target}"),
            // Bare query references are admitted only as UNION operands.
            Shape::Reference if index == 49 => "SELECT ID FROM Data".into(),
            Shape::Reference => format!("{target} UNION ALL SELECT ID FROM Data WHERE False"),
            Shape::Union => {
                format!("SELECT ID FROM {target} UNION ALL SELECT ID FROM Data WHERE False")
            }
            Shape::UnionLater => {
                format!("SELECT ID FROM Data WHERE False UNION ALL SELECT ID FROM {target}")
            }
            Shape::MixedInline => format!("SELECT ID FROM (SELECT ID FROM {target}) A"),
            Shape::FixedCrosstab => {
                format!("TRANSFORM Count(*) SELECT ID FROM {target} GROUP BY ID PIVOT 1 IN (1)")
            }
            Shape::DynamicCrosstab => {
                format!("TRANSFORM Count(*) SELECT ID FROM {target} GROUP BY ID PIVOT 1")
            }
        };
        *query = syntax(&sql);
    }
    source
}

fn checked_limit(shape: Shape, phase: &str, error: QueryError) {
    assert!(
        matches!(error, QueryError::StructureLimit | QueryError::Limit(_)),
        "{shape:?} {phase} returned an unexpected refusal: {error:?}"
    );
    eprintln!("{shape:?}: checked {phase} refusal {error:?}; not full depth-50 support");
}

fn run_shape(shape: Shape) {
    let mut source = shape_chain(shape);
    source.diagnostics = matches!(shape, Shape::UnionLater);
    eprintln!("{shape:?}: metadata preparation starts");
    let query = syntax("SELECT ID FROM Q0");
    if matches!(shape, Shape::DynamicCrosstab) {
        match execute_with_schema_discovery(&query, &source, &[], Default::default()) {
            Ok(result) => {
                assert_eq!(
                    result.rows(),
                    [
                        vec![ExpressionValue::Value(Value::Int32(1))],
                        vec![ExpressionValue::Value(Value::Int32(2))],
                    ]
                );
                eprintln!("{shape:?}: depth-50 staged execution completed");
            }
            Err(error) => checked_limit(shape, "discovery/execution", error),
        }
        return;
    }
    let plan = match prepare(&query, &source) {
        Ok(plan) => plan,
        Err(error) => {
            assert_eq!(source.scans.get(), 0);
            checked_limit(shape, "metadata preparation", error);
            return;
        }
    };
    assert_eq!(source.scans.get(), 0);
    eprintln!("{shape:?}: preparation complete, execution starts");
    match plan.execute(&source, &[], Default::default()) {
        Ok(result) => {
            assert_eq!(
                result.rows(),
                [vec![Value::Int32(1)], vec![Value::Int32(2)]]
            );
            eprintln!("{shape:?}: depth-50 preparation and execution completed");
        }
        Err(error) => checked_limit(shape, "execution", error),
    }
}

#[test]
fn table_body_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::table_body_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::Table),
    );
}

#[test]
fn reference_body_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::reference_body_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::Reference),
    );
}

#[test]
fn union_first_arm_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::union_first_arm_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::Union),
    );
}

#[test]
fn union_later_arm_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::union_later_arm_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::UnionLater),
    );
}

#[test]
fn fixed_crosstab_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::fixed_crosstab_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::FixedCrosstab),
    );
}

#[test]
fn dynamic_crosstab_chain_completes_or_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::dynamic_crosstab_chain_completes_or_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::DynamicCrosstab),
    );
}

fn run_domain_composition() {
    let mut source = Definitions::chain(50);
    // Domain target resolution is runtime work, so this recursion must reach
    // the separate domain/work ceiling rather than the named dependency cycle.
    source.queries[49].1 = syntax("SELECT DCount('*','Q0') AS ID FROM Data");
    source.diagnostics = true;
    eprintln!("domain probe: metadata preparation starts");
    let plan = prepare(&syntax("SELECT ID FROM Q0"), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let error = plan
        .execute(&source, &[], Default::default())
        .err()
        .unwrap();
    assert!(
        matches!(error, QueryError::StructureLimit | QueryError::Limit(_)),
        "deep named/domain recursion must fail through a checked limit: {error:?}"
    );
    eprintln!("named/domain composition: checked runtime refusal {error:?}");
}

#[test]
fn deep_named_domain_composition_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::deep_named_domain_composition_reaches_a_checked_limit_without_stack_abort",
        run_domain_composition,
    );
}

#[test]
fn suspended_inline_syntax_reaches_a_checked_limit_without_stack_abort() {
    isolated_worker(
        "shapes::suspended_inline_syntax_reaches_a_checked_limit_without_stack_abort",
        || run_shape(Shape::MixedInline),
    );
}

fn run_staged_domain_composition() {
    let mut source = Definitions::chain(50);
    source.queries[49].1 = syntax("SELECT DCount('*','Dyn') AS ID FROM Data");
    source.queries.push((
        "Dyn".into(),
        syntax("TRANSFORM Sum(DCount('*','R0')) SELECT ID FROM Data GROUP BY ID PIVOT 1"),
    ));
    for prefix in ['R', 'S'] {
        for index in 0..50 {
            let sql = if index == 49 {
                if prefix == 'R' {
                    "SELECT DCount('*','S0') AS ID FROM Data".to_owned()
                } else {
                    "SELECT ID FROM Data".to_owned()
                }
            } else {
                format!("SELECT ID FROM {prefix}{}", index + 1)
            };
            source
                .queries
                .push((format!("{prefix}{index}"), syntax(&sql)));
        }
    }
    source.diagnostics = true;
    eprintln!("staged domain probe: preparation/execution starts");
    let result = execute_with_schema_discovery(
        &syntax("SELECT ID FROM Q0"),
        &source,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 128 * 1024 * 1024,
                rows: 100_000,
                steps: 10_000_000,
            },
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::StructureLimit));
    eprintln!("staged domain probe: shared active-interpreter bound refused safely");
}

#[test]
fn staged_domain_runtime_keeps_the_active_interpreter_allowance() {
    isolated_worker(
        "shapes::staged_domain_runtime_keeps_the_active_interpreter_allowance",
        run_staged_domain_composition,
    );
}

fn staged_domains(count: usize) -> Definitions {
    let mut source = Definitions::chain(0);
    for index in 0..count {
        let transform = if index + 1 == count {
            "Count(*)".to_owned()
        } else {
            format!("Sum(DCount('*','C{}'))", index + 1)
        };
        source.queries.push((
            format!("C{index}"),
            syntax(&format!(
                "TRANSFORM {transform} SELECT ID FROM Data GROUP BY ID PIVOT 1"
            )),
        ));
    }
    source
}

fn run_shared_domain_limit() {
    let source = staged_domains(16);
    let result = execute_with_schema_discovery(
        &syntax("SELECT DCount('*','C0'),DCount('*','C0')"),
        &source,
        &[],
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            ExpressionValue::Value(Value::Int64(2)),
            ExpressionValue::Value(Value::Int64(2)),
        ]]
    );

    let source = staged_domains(17);
    let error = execute_with_schema_discovery(
        &syntax("SELECT DCount('*','C0')"),
        &source,
        &[],
        Default::default(),
    )
    .err()
    .unwrap();
    assert_eq!(error, QueryError::StructureLimit);
    // The seventeenth domain call refuses before its target can scan. These
    // chains remain below the separate named-fifty/read-128 ceilings.
    assert_eq!(source.scans.get(), 16);
    eprintln!("staged domain scope: sixteen succeeds twice; seventeen refuses before target scan");
}

#[test]
fn staged_domains_share_sixteen_call_limit_and_release_sequential_scopes() {
    isolated_worker(
        "shapes::staged_domains_share_sixteen_call_limit_and_release_sequential_scopes",
        run_shared_domain_limit,
    );
}
