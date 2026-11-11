//! Formatting must preserve admission under the same caller-issued depth bound.
use crate::{ParseLimits, parse_query};

fn check_round_trip(expression: &str, max_depth: usize) {
    let sql =
        format!("CREATE TABLE Samples (Active BIT CONSTRAINT ActiveRule CHECK ({expression}))");
    round_trip(&sql, max_depth);
}

fn round_trip(sql: &str, max_depth: usize) -> String {
    let limits = ParseLimits {
        max_depth,
        ..Default::default()
    };
    let original = parse_query(sql, limits).expect("source expression fits the depth bound");
    let normalized = original.query().to_string();
    let reparsed = parse_query(&normalized, limits).unwrap_or_else(|error| {
        panic!(
            "normalized declaration exceeded its admitted depth {max_depth}: {error}; {normalized}"
        )
    });

    assert_eq!(original.query(), reparsed.query());
    assert_eq!(reparsed.query().to_string(), normalized);
    normalized
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

#[test]
fn create_check_nested_read_query_normalization_retains_the_admitted_depth_boundary() {
    for query_depth in [1, 4] {
        let expression = format!(
            "{}{}True{}",
            "(SELECT ".repeat(query_depth),
            "NOT ".repeat(30),
            ")".repeat(query_depth)
        );
        check_round_trip(&expression, 32);
    }
}

#[test]
fn annotation_precedence_preserves_operator_grouping_without_changing_plain_queries() {
    for expression in [
        "A - (B - C)",
        "(A - B) - C",
        "A / (B / C)",
        "(A ^ B) ^ C",
        "A ^ (B ^ C)",
        "(-2) ^ 2",
        "-2 ^ 2",
        "-(-Amount)",
        "NOT (A AND B)",
        "(NOT A) = B",
        "NOT A = B",
        "-(NOT Active)",
        "A BETWEEN (B AND C) AND (D OR E)",
        "(A OR B) LIKE (C OR D)",
        "A NOT IN (NOT B, C - (D - E), CBool(NOT F))",
        "A = ANY (SELECT B - (C - D) FROM Limits WHERE NOT [Ready] AND [Enabled])",
        "EXISTS (SELECT CBool(NOT A) FROM (SELECT NOT B AS A) AS Nested WHERE NOT A ORDER BY NOT A)",
    ] {
        check_round_trip(expression, 32);
        let sql = format!("CREATE TABLE Samples (Amount LONG DEFAULT {expression})");
        round_trip(&sql, 32);
    }

    let normalized = round_trip("CREATE TABLE Samples (Amount LONG DEFAULT -(-Amount))", 32);
    assert!(normalized.contains("DEFAULT - - [Amount]"));
    assert!(!normalized.contains("--"));
    let query = parse_query("SELECT A - (B - C)", ParseLimits::default()).unwrap();
    assert_eq!(query.query().to_string(), "SELECT ([A] - ([B] - [C]));");
}

#[test]
fn annotation_nested_calls_and_right_associated_operators_retain_their_owned_structure() {
    let call = format!(
        "{}{}True{}",
        "CBool(".repeat(8),
        "NOT ".repeat(23),
        ")".repeat(8)
    );
    check_round_trip(&call, 32);

    let mut right_associated = String::from("A");
    for _ in 0..20 {
        right_associated = format!("A - ({right_associated})");
    }
    check_round_trip(&right_associated, 32);
}
