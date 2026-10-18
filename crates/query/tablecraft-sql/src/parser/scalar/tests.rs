use super::*;

#[test]
fn field_rules_expand_comparisons_without_rewriting_literals_or_explicit_names() {
    for (rule, expected) in [
        (">=0 And <=100", "[Score] >= 0 And [Score] <= 100"),
        (
            "(>0 Or =-1) And Is Not Null",
            "([Score] > 0 Or [Score] = -1) And [Score] Is Not Null",
        ),
        (
            "[Score] > 0 And [Score] < 100",
            "[Score] > 0 And [Score] < 100",
        ),
        ("<> 'And > 0'", "[Score] <> 'And > 0'"),
    ] {
        assert_eq!(
            parse_field_rule(rule, "Score", ParseLimits::default()).unwrap(),
            parse_expression(expected, ParseLimits::default()).unwrap()
        );
    }
}

#[test]
fn names_are_single_tokens_and_extra_statements_or_expansion_overflow_fail() {
    let expression = parse_field_rule(">0", "odd And > Name", ParseLimits::default()).unwrap();
    let Expression::Binary { left, .. } = expression else {
        panic!("comparison expected")
    };
    let Expression::Field(field) = *left else {
        panic!("field expected")
    };
    assert_eq!(field.name.as_str(), "odd And > Name");
    for sql in ["1; DELETE FROM T", "1 FROM T", "SELECT 1", "1,2"] {
        assert!(parse_expression(sql, ParseLimits::default()).is_err());
    }
    assert!(parse_field_rule(">0", "", ParseLimits::default()).is_err());
    assert!(parse_field_rule(">0", "bad]name", ParseLimits::default()).is_err());
    assert!(
        parse_field_rule(
            ">0",
            "X",
            ParseLimits {
                max_tokens: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_field_rule(
            ">0",
            "LongName",
            ParseLimits {
                max_bytes: 3,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        parse_expression(
            "((((1))))",
            ParseLimits {
                max_depth: 2,
                ..Default::default()
            }
        )
        .is_err()
    );
}
