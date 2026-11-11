//! Formatting must preserve admission under the same caller-issued depth bound.
use crate::{ParseLimits, parse_query};

fn check_round_trip(expression: &str, max_depth: usize) {
    let limits = ParseLimits {
        max_depth,
        ..Default::default()
    };
    let sql =
        format!("CREATE TABLE Samples (Active BIT CONSTRAINT ActiveRule CHECK ({expression}))");
    let original = parse_query(&sql, limits).expect("source expression fits the depth bound");
    let normalized = original.query().to_string();
    let reparsed = parse_query(&normalized, limits).unwrap_or_else(|error| {
        panic!(
            "normalized declaration exceeded its admitted depth {max_depth}: {error}; {normalized}"
        )
    });

    assert_eq!(original.query(), reparsed.query());
    assert_eq!(reparsed.query().to_string(), normalized);
}

#[test]
fn create_check_unary_normalization_retains_the_admitted_depth_boundary() {
    for max_depth in [32, 4] {
        let expression = format!("{}True", "NOT ".repeat(max_depth - 1));
        check_round_trip(&expression, max_depth);
    }
}

#[test]
fn create_check_call_argument_normalization_retains_the_admitted_depth_boundary() {
    for max_depth in [32, 4] {
        let expression = format!("CBool({}True)", "NOT ".repeat(max_depth - 2));
        check_round_trip(&expression, max_depth);
    }
}

#[test]
fn create_check_binary_chain_normalization_retains_the_admitted_depth_boundary() {
    for max_depth in [32, 4] {
        let expression = vec!["True"; max_depth].join(" AND ");
        check_round_trip(&expression, max_depth);
    }
}
