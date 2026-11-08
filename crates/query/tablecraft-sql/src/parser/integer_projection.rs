//! Retain lexical evidence before integer spelling and parentheses are lowered.
use super::lex;
use sqlparser::tokenizer::{Token, TokenWithSpan};
use std::ops::Range;

const MAX_INT32_DIGITS: usize = 10;

pub(super) fn spelling(source: &str, tokens: &[TokenWithSpan]) -> Option<Range<usize>> {
    let (select, mut rest) = tokens.split_first()?;
    if !word(&select.token, "SELECT") {
        return None;
    }
    if rest.first().is_some_and(|token| word(&token.token, "ALL")) {
        rest = &rest[1..];
    }
    let [number, alias_keyword, alias, tail @ ..] = rest else {
        return None;
    };
    if !matches!(number.token, Token::Number(_, false))
        || !word(&alias_keyword.token, "AS")
        || !matches!(alias.token, Token::Word(_))
        || !(tail.is_empty() || matches!(tail, [token] if token.token == Token::SemiColon))
    {
        return None;
    }
    let range =
        lex::byte_offset(source, number.span.start)..lex::byte_offset(source, number.span.end);
    let literal = source.get(range.clone())?;
    if literal.is_empty()
        || literal.len() > MAX_INT32_DIGITS
        || !literal.bytes().all(|byte| byte.is_ascii_digit())
        || (literal.len() > 1 && literal.starts_with('0'))
        || literal.parse::<i32>().is_err()
    {
        return None;
    }
    Some(range)
}

fn word(token: &Token, expected: &str) -> bool {
    matches!(token, Token::Word(word) if word.quote_style.is_none() && word.value.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use crate::{ParseLimits, parse_query};

    #[test]
    fn canonical_projection_spelling_preserves_trivia_without_normalizing_expression() {
        for (sql, expected) in [
            ("SELECT 0 AS Answer", "0"),
            (" select ALL 43 as [Answer]; ", "43"),
            ("SELECT /* é */ 2147483647 AS Answer;", "2147483647"),
            ("SELECT\n42\nAS Answer; -- trailing", "42"),
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                parsed.canonical_integer_projection_spelling(),
                Some(expected)
            );
        }
    }

    #[test]
    fn normalized_integer_ast_never_substitutes_for_original_spelling() {
        for sql in [
            "SELECT 043 AS Answer;",
            "SELECT 00 AS Answer;",
            "SELECT (43) AS Answer;",
            "SELECT +43 AS Answer;",
            "SELECT -0 AS Answer;",
            "SELECT 43.0 AS Answer;",
            "SELECT 43e0 AS Answer;",
            "SELECT &H2B AS Answer;",
            "SELECT 2147483648 AS Answer;",
            "SELECT 43 AS Answer FROM Items;",
            "SELECT TOP 1 43 AS Answer;",
            "SELECT 43 AS Answer ORDER BY 1;",
            "SELECT 43 AS Answer UNION SELECT 44 AS Answer;",
        ] {
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            assert_eq!(
                parsed.canonical_integer_projection_spelling(),
                None,
                "{sql}"
            );
        }
    }
}
