use proptest::prelude::*;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Sales".into(),
            vec![
                Column::new("Region".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Quarter".into(), ScalarType::Int32, true).unwrap(),
                Column::new("Amount".into(), ScalarType::Int32, true).unwrap(),
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::Int32(2),
                    Value::Int32(10),
                    Value::Text("unused".into()),
                ],
                vec![
                    Value::Int32(1),
                    Value::Int32(1),
                    Value::Int32(3),
                    Value::Text("unused".into()),
                ],
                vec![
                    Value::Int32(1),
                    Value::Int32(2),
                    Value::Int32(7),
                    Value::Text("unused".into()),
                ],
                vec![
                    Value::Int32(2),
                    Value::Int32(3),
                    Value::Int32(5),
                    Value::Text("unused".into()),
                ],
                vec![
                    Value::Int32(2),
                    Value::Int32(1),
                    Value::Null,
                    Value::Text("unused".into()),
                ],
                vec![
                    Value::Int32(3),
                    Value::Null,
                    Value::Int32(8),
                    Value::Text("unused".into()),
                ],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(query.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
#[test]
fn dynamic_cells_totals_and_sorted_headings_retain_full_row_groups() {
    let result=run("TRANSFORM Sum(Amount) SELECT Region,Sum(Amount) AS Total FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter",&source()).unwrap();
    assert_eq!(result.columns(), ["Region", "Total", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [
            vec![
                Value::Int32(1),
                Value::Int64(20),
                Value::Int64(3),
                Value::Int64(17),
                Value::Null
            ],
            vec![
                Value::Int32(2),
                Value::Int64(5),
                Value::Null,
                Value::Null,
                Value::Int64(5)
            ],
            vec![
                Value::Int32(3),
                Value::Int64(8),
                Value::Null,
                Value::Null,
                Value::Null
            ],
        ]
    );
}
#[test]
fn fixed_headings_keep_order_missing_columns_and_unselected_values_in_totals() {
    let result=run("TRANSFORM Count(*) SELECT Region,Sum(Amount) AS Total FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (3,2,4)",&source()).unwrap();
    assert_eq!(result.columns(), ["Region", "Total", "3", "2", "4"]);
    assert_eq!(
        result.rows(),
        [
            vec![
                Value::Int32(1),
                Value::Int64(20),
                Value::Null,
                Value::Int64(2),
                Value::Null
            ],
            vec![
                Value::Int32(2),
                Value::Int64(5),
                Value::Int64(1),
                Value::Null,
                Value::Null
            ],
            vec![
                Value::Int32(3),
                Value::Int64(8),
                Value::Null,
                Value::Null,
                Value::Null
            ],
        ]
    );
}
#[test]
fn where_and_having_filter_rows_without_shrinking_discovered_heading_domain() {
    let result=run("TRANSFORM Sum(Amount) SELECT Region FROM Sales WHERE Quarter=2 GROUP BY Region HAVING Sum(Amount)>10 ORDER BY Region PIVOT Quarter",&source()).unwrap();
    assert_eq!(result.columns(), ["Region", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int32(1),
            Value::Null,
            Value::Int64(17),
            Value::Null
        ]]
    );
    let empty=run("TRANSFORM Sum(Amount) SELECT Region FROM Sales WHERE Region<0 GROUP BY Region PIVOT Quarter",&source()).unwrap();
    assert_eq!(empty.columns(), ["Region", "1", "2", "3"]);
    assert!(empty.rows().is_empty());
}
#[test]
fn text_pivots_require_explicit_comparison_and_do_not_rename_case_collisions() {
    let sql = "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT IIf(Quarter=2,'B','A') IN ('B','A','C')";
    assert_eq!(
        run(sql, &source()).err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let source = source();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["Region", "B", "A", "C"]);
    let duplicate = parse_query(
        "TRANSFORM Count(*) SELECT Region FROM Sales GROUP BY Region PIVOT Unused IN ('a','A')",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(duplicate.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::AmbiguousName)
    );
}
#[test]
fn invalid_transform_shapes_names_and_limits_never_return_partial_results() {
    for sql in [
        "TRANSFORM Amount SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (1,1)",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN (NULL)",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter IN ('')",
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY 2 PIVOT Quarter",
    ] {
        assert!(run(sql, &source()).is_err(), "{sql}");
    }
    let parsed = parse_query(
        "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region PIVOT Quarter",
        ParseLimits::default(),
    )
    .unwrap();
    let source = source();
    let prepared = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| true),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::Cancelled)
    );
    assert!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    limits: tablecraft_query::QueryLimits {
                        steps: 10,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
    );
}
proptest! {
    #[test]
    fn generated_sum_grid_matches_an_independent_integer_model(values in prop::collection::vec((0_i32..4,0_i32..4,-100_i32..100),0..30)) {
        let columns=["R","C","V"].map(|name|Column::new(name.into(),ScalarType::Int32,false).unwrap()).to_vec();
        let rows=values.iter().map(|(r,c,v)|vec![Value::Int32(*r),Value::Int32(*c),Value::Int32(*v)]).collect();
        let source=MemorySource::new(vec![MemoryTable::new("T".into(),columns,rows).unwrap()]).unwrap();
        let result=run("TRANSFORM Sum(V) SELECT R FROM T GROUP BY R ORDER BY R PIVOT C IN (0,1,2,3)",&source).unwrap();
        let mut model=std::collections::BTreeMap::<i32,[Option<i64>;4]>::new();
        for(r,c,v)in values {let cell=&mut model.entry(r).or_insert([None;4])[c as usize];*cell=Some(cell.unwrap_or(0)+i64::from(v));}
        let expected=model.into_iter().map(|(r,cells)|std::iter::once(Value::Int32(r)).chain(cells.map(|v|v.map_or(Value::Null,Value::Int64))).collect::<Vec<_>>()).collect::<Vec<_>>();
        prop_assert_eq!(result.rows(),expected);
    }
}

#[test]
fn fixed_parameters_reuse_a_plan_without_stale_column_names() {
    let parsed=parse_query("PARAMETERS Chosen LONG; TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (Chosen)",ParseLimits::default()).unwrap();
    let source = source();
    let prepared = prepare(parsed.query(), &source).unwrap();
    for (chosen, first) in [(2, 17), (1, 3)] {
        let result = prepared
            .execute(
                &source,
                &[("Chosen", Value::Int32(chosen))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(result.columns(), ["Region", &chosen.to_string()]);
        assert_eq!(result.rows()[0], [Value::Int32(1), Value::Int64(first)]);
    }
}
#[test]
fn nonempty_null_operands_are_distinct_from_missing_intersections() {
    let result=run("TRANSFORM Count(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (1,2,3)",&source()).unwrap();
    assert_eq!(
        result.rows()[1],
        [
            Value::Int32(2),
            Value::Int64(0),
            Value::Null,
            Value::Int64(1)
        ]
    );
    let result=run("TRANSFORM Nz(Sum(Amount),0) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter IN (1,2,3)",&source()).unwrap();
    assert_eq!(
        result.rows()[1],
        [
            Value::Int32(2),
            Value::Int64(0),
            Value::Null,
            Value::Int64(5)
        ]
    );
}
#[test]
fn discovery_has_a_column_ceiling_and_metadata_binding_does_not_scan() {
    struct Metadata {
        columns: Vec<Column>,
    }
    impl Source for Metadata {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("binding or invalid fixed headings scanned records")
        }
    }
    let source = Metadata {
        columns: vec![Column::new("V".into(), ScalarType::Int32, false).unwrap()],
    };
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT 1 AS R FROM T GROUP BY 1 PIVOT V",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(prepare(parsed.query(), &source).is_ok());
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT 1 AS R FROM T GROUP BY 1 PIVOT V IN (1,1)",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::AmbiguousName)
    );
    let columns = vec![Column::new("V".into(), ScalarType::Int32, false).unwrap()];
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            columns,
            (0..256).map(|n| vec![Value::Int32(n)]).collect(),
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(
        run(
            "TRANSFORM Count(*) SELECT 1 AS R FROM T GROUP BY 1 PIVOT V",
            &source
        )
        .err(),
        Some(QueryError::StructureLimit)
    );
}
#[test]
fn fixed_pivot_evaluation_follows_where_but_dynamic_discovery_retains_its_domain() {
    let fixed = "TRANSFORM Count(*) SELECT Region FROM Sales WHERE Quarter<>2 GROUP BY Region PIVOT 10 \\ (Quarter-2) IN (-10,10)";
    assert!(run(fixed, &source()).is_ok());
    let dynamic = "TRANSFORM Count(*) SELECT Region FROM Sales WHERE Quarter<>2 GROUP BY Region PIVOT 10 \\ (Quarter-2)";
    assert_eq!(
        run(dynamic, &source()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn pruning_rewrites_noncontiguous_pivot_only_and_aggregate_only_slots() {
    let columns = ["UnusedA", "R", "UnusedB", "V", "C"]
        .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
        .to_vec();
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            columns,
            vec![
                vec![
                    Value::Int32(99),
                    Value::Int32(1),
                    Value::Int32(88),
                    Value::Int32(7),
                    Value::Int32(2),
                ],
                vec![
                    Value::Int32(99),
                    Value::Int32(1),
                    Value::Int32(88),
                    Value::Int32(3),
                    Value::Int32(1),
                ],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let result = run(
        "TRANSFORM Sum(V) SELECT R FROM T GROUP BY R PIVOT C IN (1,2)",
        &source,
    )
    .unwrap();
    assert_eq!(result.columns(), ["R", "1", "2"]);
    assert_eq!(
        result.rows(),
        [vec![Value::Int32(1), Value::Int64(3), Value::Int64(7)]]
    );
}

#[test]
fn aggregate_alias_does_not_replace_pivot_headings_or_grant_owner_authority() {
    let plain = "TRANSFORM Sum(Amount) SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter";
    let alias = "TRANSFORM Sum(Amount) AS Measure SELECT Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter";
    let source = source();
    let expected = run(plain, &source).unwrap();
    let actual = run(alias, &source).unwrap();
    assert_eq!(actual.columns(), expected.columns());
    assert_eq!(actual.rows(), expected.rows());
    assert!(matches!(
        run(&format!("{alias} WITH OWNERACCESS OPTION"), &source),
        Err(QueryError::Unsupported)
    ));
}

#[test]
fn crosstab_top_counts_percentages_and_complete_sort_ties_limit_groups_not_headings() {
    let source = source();
    let base = "TRANSFORM Sum(Amount) SELECT {top} Region,Sum(Amount) AS Total FROM Sales {filter} GROUP BY Region {having} {sort} PIVOT Quarter";
    for (top, filter, having, sort, ids) in [
        ("TOP 1", "", "", "ORDER BY Total DESC", vec![1]),
        ("TOP 1", "", "", "ORDER BY Region Mod 2 DESC", vec![1, 3]),
        (
            "TOP 1",
            "",
            "",
            "ORDER BY Region Mod 2 DESC,Region",
            vec![1],
        ),
        ("TOP 34 PERCENT", "", "", "ORDER BY Region", vec![1, 2]),
        ("TOP 100 PERCENT", "", "", "ORDER BY Region", vec![1, 2, 3]),
        ("TOP 4294967295", "", "", "ORDER BY Region", vec![1, 2, 3]),
        ("TOP 2", "", "", "", vec![1, 2]),
        (
            "TOP 50 PERCENT",
            "",
            "HAVING Sum(Amount)<10",
            "ORDER BY Region",
            vec![2],
        ),
        ("TOP 1", "WHERE Region<0", "", "ORDER BY Region", vec![]),
    ] {
        let sql = base
            .replace("{top}", top)
            .replace("{filter}", filter)
            .replace("{having}", having)
            .replace("{sort}", sort);
        let result = run(&sql, &source).unwrap();
        assert_eq!(result.columns(), ["Region", "Total", "1", "2", "3"]);
        assert_eq!(
            result
                .rows()
                .iter()
                .map(|row| match row[0] {
                    Value::Int32(id) => id,
                    _ => panic!(),
                })
                .collect::<Vec<_>>(),
            ids,
            "{sql}"
        );
    }
    for top in [
        "TOP 0",
        "TOP 0 PERCENT",
        "TOP 101 PERCENT",
        "DISTINCT TOP 1",
        "DISTINCTROW TOP 1",
    ] {
        let sql = base
            .replace("{top}", top)
            .replace("{filter}", "")
            .replace("{having}", "")
            .replace("{sort}", "");
        assert!(parse_query(&sql, Default::default()).is_err() || run(&sql, &source).is_err());
    }
}
#[test]
fn top_never_hides_errors_in_excluded_crosstab_cells_totals_having_or_order_keys() {
    let source = source();
    for sql in [
        "TRANSFORM Sum(Amount/(Region-2)) SELECT TOP 1 Region FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter",
        "TRANSFORM Sum(Amount) SELECT TOP 1 Region,Sum(Amount/(Region-2)) AS Total FROM Sales GROUP BY Region ORDER BY Region PIVOT Quarter",
        "TRANSFORM Sum(Amount) SELECT TOP 1 Region FROM Sales GROUP BY Region HAVING 1/(Region-2)>0 ORDER BY Region PIVOT Quarter",
        "TRANSFORM Sum(Amount) SELECT TOP 1 Region FROM Sales GROUP BY Region ORDER BY 1/(Region-2) PIVOT Quarter",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
}

#[test]
fn top_tie_provider_failures_and_final_tie_cancellation_are_not_suppressed() {
    use std::cell::Cell;
    struct Provider<'a> {
        calls: Cell<usize>,
        fail_at: Option<usize>,
        cancel_at: Option<usize>,
        cancelled: &'a Cell<bool>,
    }
    impl Collation for Provider<'_> {
        fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            if self.fail_at == Some(call) {
                return Err(EvalError::Domain);
            }
            if self.cancel_at == Some(call) {
                self.cancelled.set(true);
            }
            Ok(left.cmp(right))
        }
    }
    let source = source();
    let cancelled = Cell::new(false);
    let provider = Provider {
        calls: Cell::new(0),
        fail_at: None,
        cancel_at: None,
        cancelled: &cancelled,
    };
    let base = "TRANSFORM Count(*) SELECT {top} Region FROM Sales GROUP BY Region ORDER BY 'same' PIVOT Quarter";
    let query = parse_query(&base.replace("{top}", ""), Default::default()).unwrap();
    prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&provider),
                ..Default::default()
            },
        )
        .unwrap();
    let sorting_calls = provider.calls.get();
    assert!(sorting_calls > 0);
    let query = parse_query(&base.replace("{top}", "TOP 1"), Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let provider = Provider {
        calls: Cell::new(0),
        fail_at: Some(sorting_calls + 1),
        cancel_at: None,
        cancelled: &cancelled,
    };
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&provider),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::Domain))
    );
    let provider = Provider {
        calls: Cell::new(0),
        fail_at: None,
        cancel_at: Some(sorting_calls + 2),
        cancelled: &cancelled,
    };
    let cancel = || cancelled.get();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&provider),
                cancelled: Some(&cancel),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    assert_eq!(provider.calls.get(), sorting_calls + 2);
}

proptest! {
    #[test]
    fn generated_crosstab_top_matches_independent_nullable_rank_thresholds(
        scores in prop::collection::vec(prop::option::of(-3i32..4),1..12),
        value in 1u32..101, percent in any::<bool>(), secondary in any::<bool>(),
    ) {
        let count=if percent {value} else {value%10+1};
        let source=MemorySource::new(vec![MemoryTable::new("Scores".into(),vec![Column::new("Id".into(),ScalarType::Int32,false).unwrap(),Column::new("Amount".into(),ScalarType::Int32,true).unwrap()],scores.iter().enumerate().map(|(id,score)|vec![Value::Int32(id as i32),score.map_or(Value::Null,Value::Int32)]).collect()).unwrap()]).unwrap();
        let sql=format!("TRANSFORM Sum(Amount) SELECT TOP {count} {} Id,Sum(Amount) AS Total FROM Scores GROUP BY Id ORDER BY Total DESC{} PIVOT 1",if percent {"PERCENT"} else {""},if secondary {",Id"} else {""});
        let actual=run(&sql,&source).unwrap();
        let mut ranked:Vec<_>=(0..scores.len()).collect();ranked.sort_by(|left,right|scores[*right].cmp(&scores[*left]));
        let desired=if percent {(0..scores.len()).filter(|position| *position*100 < scores.len()*count as usize).count()} else {count as usize};
        let cutoff=ranked.get(desired.saturating_sub(1)).map(|index|scores[*index]);
        let selected:Vec<_>=ranked.into_iter().enumerate().filter(|(position,index)|*position<desired || (!secondary && cutoff.is_some_and(|cutoff|scores[*index]>=cutoff))).map(|(_,index)|vec![Value::Int32(index as i32),scores[index].map_or(Value::Null,|n|Value::Int64(i64::from(n))),scores[index].map_or(Value::Null,|n|Value::Int64(i64::from(n)))]).collect();
        prop_assert_eq!(actual.columns(),["Id","Total","1"]);prop_assert_eq!(actual.rows(),selected);
    }
}
