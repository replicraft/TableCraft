use super::*;
use crate::{ParseErrorKind, parse_expression_strict};

#[test]
fn qbe_aliases_use_identifier_tokens_and_preserve_the_operand_suffix() {
    for (text, alias, operand) in [
        ("Heading: [Score]+1", "Heading", "[Score]+1"),
        ("[A:B]: [Odd:Field]", "A:B", "[Odd:Field]"),
        (
            "[名称]: #2024-01-01 12:30:00#",
            "名称",
            "#2024-01-01 12:30:00#",
        ),
        ("Label: Sum(", "Label", "Sum("),
        ("[Result]: IIf('", "Result", "IIf('"),
    ] {
        let (actual, suffix) = parse_qbe_field_prefix(text, ParseLimits::default()).unwrap();
        assert_eq!(actual.as_deref(), Some(alias));
        assert_eq!(suffix, operand);
    }
    for text in [
        "[A:B]",
        "'a:b'",
        "#2024-01-01 12:30:00#",
        "[t].[A:B]",
        "IIf(True,'a:b','c')",
    ] {
        assert_eq!(
            parse_qbe_field_prefix(text, ParseLimits::default()).unwrap(),
            (None, text)
        );
    }
    assert!(parse_expression_strict("Label: 1", ParseLimits::default()).is_err());
}

#[test]
fn qbe_prefix_errors_and_limits_remain_typed() {
    assert!(parse_qbe_field_prefix("[]: 1", ParseLimits::default()).is_err());
    assert!(matches!(
        parse_qbe_field_prefix("Label: 1", ParseLimits { max_bytes: 2, ..Default::default() }),
        Err(error) if error.kind == ParseErrorKind::Limit
    ));
    assert!(matches!(
        parse_qbe_field_prefix("Label: 1 + 2", ParseLimits { max_tokens: 2, ..Default::default() }),
        Err(error) if error.kind == ParseErrorKind::Limit
    ));
}
