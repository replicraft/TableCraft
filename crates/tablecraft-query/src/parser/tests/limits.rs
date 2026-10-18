use super::*;

fn limited(sql: &str, limits: ParseLimits) {
    let error = parse_select(sql, limits).unwrap_err();
    assert_eq!(error.kind, ParseErrorKind::Limit, "{error}");
}

#[test]
fn byte_and_token_limits_are_checked_at_the_boundary() {
    let sql = "SELECT * FROM Samples";
    limited(
        sql,
        ParseLimits {
            max_bytes: sql.len() - 1,
            ..Default::default()
        },
    );
    assert!(
        parse_select(
            sql,
            ParseLimits {
                max_bytes: sql.len(),
                max_tokens: 4,
                ..Default::default()
            }
        )
        .is_ok()
    );
    limited(
        sql,
        ParseLimits {
            max_tokens: 3,
            ..Default::default()
        },
    );
    limited(
        &" ".repeat(65_537),
        ParseLimits {
            max_bytes: usize::MAX,
            ..Default::default()
        },
    );
    let columns = vec!["Code"; 1_025].join(",");
    limited(
        &format!("SELECT {columns} FROM Samples"),
        ParseLimits {
            max_tokens: usize::MAX,
            ..Default::default()
        },
    );
}

#[test]
fn limits_cover_parentheses_and_unparenthesized_expression_chains() {
    let sql = format!(
        "SELECT * FROM Samples WHERE {}Code = 1{}",
        "(".repeat(33),
        ")".repeat(33)
    );
    limited(
        &sql,
        ParseLimits {
            max_depth: usize::MAX,
            ..Default::default()
        },
    );
    let comparisons = vec!["Code = 1"; 40].join(" AND ");
    limited(
        &format!("SELECT * FROM Samples WHERE {comparisons}"),
        ParseLimits::default(),
    );
    let unary = format!("SELECT * FROM Samples WHERE {}Code", "NOT ".repeat(150));
    assert!(parse_select(&unary, ParseLimits::default()).is_err());
    limited(
        "SELECT * FROM Samples WHERE Code = 1",
        ParseLimits {
            max_depth: 1,
            ..Default::default()
        },
    );
    let near_limit = vec!["Code = 1"; 31].join(" AND ");
    round_trip(&format!("SELECT * FROM Samples WHERE {near_limit}"));
}
