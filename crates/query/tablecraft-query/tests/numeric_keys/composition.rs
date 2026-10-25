use super::support::{Numbers, PERMUTATIONS, number, run};
use tablecraft_core::values::Value;

#[test]
fn grouping_distinct_and_dynamic_union_have_encounter_independent_partitions() {
    for order in PERMUTATIONS {
        let source = Numbers::normal(order);
        for declared in [false, true] {
            let grouped = run(
                "SELECT K,Count(*) AS N FROM Numbers GROUP BY K ORDER BY K",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(grouped.rows().len(), 2);
            assert_eq!(number(&grouped.rows()[0][0]), 1.0);
            assert_eq!(grouped.rows()[0][1], Value::Int64(2));
            assert_eq!(number(&grouped.rows()[1][0]), 1.00000001);
            assert_eq!(grouped.rows()[1][1], Value::Int64(1));

            for sql in [
                "SELECT DISTINCT K FROM Numbers ORDER BY K",
                "SELECT K FROM Numbers UNION SELECT K FROM Numbers ORDER BY K",
            ] {
                let result = run(sql, &source, declared).unwrap();
                let values: Vec<_> = result.rows().iter().map(|row| number(&row[0])).collect();
                assert_eq!(values, [1.0, 1.00000001], "{order:?}: {sql}");
            }
        }
    }
}

#[test]
fn ordering_top_ties_and_extrema_use_the_same_exact_keys() {
    for order in PERMUTATIONS {
        let source = Numbers::normal(order);
        for declared in [false, true] {
            let result = run("SELECT K FROM Numbers ORDER BY K", &source, declared).unwrap();
            let values: Vec<_> = result.rows().iter().map(|row| number(&row[0])).collect();
            assert_eq!(values, [1.0, 1.0, 1.00000001]);
            assert_eq!(
                result
                    .rows()
                    .iter()
                    .filter(|row| matches!(row[0], Value::Single(_)))
                    .count(),
                1
            );

            let top = run(
                "SELECT TOP 1 K FROM Numbers ORDER BY K DESC",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(top.rows().len(), 1);
            assert_eq!(number(&top.rows()[0][0]), 1.00000001);
            let low = run("SELECT TOP 1 K FROM Numbers ORDER BY K", &source, declared).unwrap();
            assert_eq!(low.rows().len(), 2);

            for sql in [
                "SELECT Min(K),Max(K) FROM Numbers",
                "SELECT DMin('K','Numbers'),DMax('K','Numbers')",
            ] {
                let extrema = run(sql, &source, declared).unwrap();
                assert_eq!(number(&extrema.rows()[0][0]), 1.0);
                assert_eq!(number(&extrema.rows()[0][1]), 1.00000001);
            }
        }
    }
}

#[test]
fn dynamic_and_fixed_pivots_conserve_rows_under_the_same_heading_equivalence() {
    for order in PERMUTATIONS {
        let source = Numbers::normal(order);
        for declared in [false, true] {
            for suffix in [
                "",
                " IN (CDbl(1),CDbl(1.00000001))",
                " IN (CSng(1),CDbl(1.00000001))",
            ] {
                let sql = format!(
                    "TRANSFORM Count(*) SELECT 7 AS G FROM Numbers GROUP BY 7 PIVOT K{suffix}"
                );
                let result = run(&sql, &source, declared).unwrap();
                assert_eq!(result.columns(), ["G", "1", "1.00000001"]);
                assert_eq!(
                    result.rows(),
                    &[vec![Value::Int64(7), Value::Int64(2), Value::Int64(1)]]
                );
            }

            let sql = "TRANSFORM Count(*) SELECT K FROM Numbers GROUP BY K ORDER BY K PIVOT 7";
            let result = run(sql, &source, declared).unwrap();
            assert_eq!(result.rows().len(), 2);
            assert_eq!(result.rows()[0][1], Value::Int64(2));
            assert_eq!(result.rows()[1][1], Value::Int64(1));
        }
    }
}

#[test]
fn subnormal_and_large_finite_float_keys_do_not_narrow_to_single() {
    for (single, low, high) in [("0", "0", "1e-46"), ("3e38", "3e38", "1e300")] {
        for order in PERMUTATIONS {
            let source = Numbers::new(order, single, low, high);
            for declared in [false, true] {
                let result = run("SELECT K FROM Numbers ORDER BY K", &source, declared).unwrap();
                let values: Vec<_> = result.rows().iter().map(|row| number(&row[0])).collect();
                assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
                assert_eq!(values[2], high.parse::<f64>().unwrap());
                let top = run(
                    "SELECT TOP 1 K FROM Numbers ORDER BY K DESC",
                    &source,
                    declared,
                )
                .unwrap();
                assert_eq!(top.rows().len(), 1);
                assert_eq!(number(&top.rows()[0][0]), high.parse::<f64>().unwrap());
            }
        }
    }
}
