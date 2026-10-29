use std::{cell::RefCell, cmp::Ordering};
use tablecraft_access::{FrenchLegacyCollation, NativeCollationResolver};
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile, StringCompareMode},
    logic::Truth,
    predicates::{LikeMode, LikeString, like, like_utf16},
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000)
}

fn order() -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([12, 4, 0, 0])
}

fn policy() -> FrenchLegacyCollation<'static> {
    FrenchLegacyCollation::new(order(), budget(), &|| false).unwrap()
}

fn run(text: &str, pattern: &str, mode: LikeMode) -> Result<Truth, EvalError> {
    like(
        &Value::Text(text.into()),
        &Value::Text(pattern.into()),
        Some(mode),
        Some(&policy()),
        &mut budget(),
        None,
    )
}

#[test]
fn complete_ansi_grammars_keep_spaces_expansions_negation_and_utf16_unit_counts() {
    for (text, pattern, expected) in [
        ("côte", "CÔTE", true),
        ("cote", "côte", false),
        ("côte", "c[ô]te", true),
        ("côte", "c[o]te", false),
        ("é", "[é-è]", true),
        ("è", "[é-è]", true),
        ("ss", "[ß]", true),
        ("ss", "[!ß]*", false),
        ("ss", "[!b]", true),
        ("ß", "s[]s", true),
        ("a", "a ", false),
        ("a ", "a", false),
        ("a ", "a*", true),
        ("😀", "?", false),
        ("😀", "??", true),
        ("2", "#", true),
        ("２", "#", false),
        ("", "[]*", true),
        ("[", "[[]", true),
        ("]", "]", true),
        ("-", "[a-]", true),
        ("-", "[-a]", true),
        ("*", "[*]", true),
        ("?", "[?]", true),
        ("abc", "a%", false),
        ("\0", "?", true),
        ("", "\0", true),
        ("\0", "", false),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi89).unwrap(),
            if expected { Truth::True } else { Truth::False },
            "{text:?}/{pattern:?}"
        );
    }
    for (text, pattern, expected) in [
        ("côte", "C_Ô%", false),
        ("côte", "C_TE", true),
        ("côte", "c[^o]te", true),
        ("ss", "[^ß]%", false),
        ("ss", "[ß]", true),
        ("😀", "__", true),
        ("#", "#", true),
        ("*", "*", true),
        ("?", "?", true),
        ("a ", "a%", true),
        ("a", "a ", false),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi92).unwrap(),
            if expected { Truth::True } else { Truth::False },
            "{text:?}/{pattern:?}"
        );
    }
}

#[test]
fn pattern_callbacks_are_untrimmed_while_sql_and_search_keep_their_own_policies() {
    let provider = policy();
    assert_eq!(provider.like_profile(), LikeProfile::Utf16Weights);
    assert_eq!(provider.compare("a ", "a"), Ok(Ordering::Equal));
    assert_eq!(
        provider.compare_like_units(&[97, 32], &[97]),
        Ok(Ordering::Greater)
    );
    assert!(!provider.is_like_unit(&[97, 32]).unwrap());
    assert_eq!(
        provider.find(
            "a ",
            "a ",
            tablecraft_expressions::comparison::StringSearch::ForwardFrom(0),
            StringCompareMode::Database
        ),
        Ok(Some(0..1))
    );
    assert_eq!(
        provider.compare_text("a", "A"),
        Err(EvalError::CollationRequired)
    );
    let bytes = [0x55, 0x00, 0xd8];
    assert_eq!(
        like_utf16(
            LikeString::Utf16Le(&bytes[1..]),
            LikeString::Text("?"),
            LikeMode::Ansi89,
            &provider,
            &mut budget(),
            None
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        like_utf16(
            LikeString::Utf16Le(&bytes[1..2]),
            LikeString::Text("?"),
            LikeMode::Ansi89,
            &provider,
            &mut budget(),
            None
        ),
        Err(EvalError::UnrepresentableText)
    );
}

#[test]
fn errors_are_reached_at_existing_grammar_frontiers_and_null_does_not_activate_the_provider() {
    for (text, pattern, expected) in [
        ("x", "a[", Ok(Truth::False)),
        ("ab", "a[", Err(EvalError::InvalidPattern)),
        ("x", "*a[", Err(EvalError::InvalidPattern)),
        ("x", "*a[z-a]", Ok(Truth::False)),
        ("ab", "*a[z-a]", Err(EvalError::InvalidPattern)),
        ("", "[", Ok(Truth::False)),
        ("a", "a[", Ok(Truth::False)),
        ("", "*[", Err(EvalError::InvalidPattern)),
        ("", "*[z-a]", Ok(Truth::False)),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi89),
            expected,
            "{text:?}/{pattern:?}"
        );
    }
    let cancelled = FrenchLegacyCollation::new(order(), budget(), &|| false).unwrap();
    assert_eq!(
        like(
            &Value::Null,
            &Value::Text("[".into()),
            Some(LikeMode::Ansi89),
            Some(&cancelled),
            &mut ResourceBudget::new(0, 0, 1),
            None
        ),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        like(
            &Value::Null,
            &Value::Null,
            None,
            Some(&cancelled),
            &mut budget(),
            None
        ),
        Err(EvalError::LikeModeRequired)
    );
    assert!(matches!(
        like(
            &Value::Text("a".into()),
            &Value::Text("*".into()),
            Some(LikeMode::Ansi89),
            Some(&cancelled),
            &mut budget(),
            Some(&|| true)
        ),
        Err(EvalError::Cancelled)
    ));
}

#[test]
fn shared_provider_pool_reentrancy_and_limits_are_retained_across_like_and_comparison() {
    let pool = RefCell::new(ResourceBudget::new(4000, 0, 100_000));
    let resolver = NativeCollationResolver::new(order(), &pool, &|| false).unwrap();
    let provider = resolver.default_collation();
    {
        let _held = pool.borrow_mut();
        assert_eq!(
            provider.compare_like_units(&[97], &[97]),
            Err(EvalError::CollationFailed)
        );
        assert_eq!(
            provider.is_like_unit(&[97]),
            Err(EvalError::CollationFailed)
        );
    }
    let mut successes = 0;
    for _ in 0..100 {
        let result = like(
            &Value::Text("ss".into()),
            &Value::Text("[ß]".into()),
            Some(LikeMode::Ansi89),
            Some(provider),
            &mut budget(),
            None,
        );
        match result {
            Ok(Truth::True) => successes += 1,
            Err(EvalError::Limit(_)) => break,
            other => panic!("Unexpected result: {other:?}"),
        }
    }
    assert!(successes > 0 && successes < 100);
    assert!(matches!(
        provider.compare("a", "a"),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn derived_grouped_and_correlated_queries_retain_french_like_policy_and_eager_errors() {
    let provider = policy();
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT q.V LIKE '[ß]' AS M,Count(*) AS N FROM (SELECT 'ss' AS V UNION ALL SELECT 'ß' UNION ALL SELECT 'road') AS q GROUP BY q.V LIKE '[ß]' HAVING q.V LIKE '[ß]'",
            vec![vec![Value::Boolean(true), Value::Int64(2)]],
        ),
        (
            "SELECT q.ID FROM (SELECT 1 AS ID,'ss' AS V UNION ALL SELECT 2,'road') AS q WHERE EXISTS(SELECT 1 WHERE q.V LIKE '[ß]')",
            vec![vec![Value::Int64(1)]],
        ),
        (
            "SELECT 'é' LIKE '[é-è]', 'a' NOT LIKE 'a ', 'ss' LIKE '[!ß]*'",
            vec![vec![
                Value::Boolean(true),
                Value::Boolean(true),
                Value::Boolean(false),
            ]],
        ),
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&provider),
                    like_mode: Some(LikeMode::Ansi89),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), expected);
    }
    let parsed = parse_query("SELECT NULL LIKE (1/0)", ParseLimits::default()).unwrap();
    assert!(matches!(
        prepare(parsed.query(), &source).unwrap().execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&provider),
                like_mode: Some(LikeMode::Ansi89),
                ..Default::default()
            }
        ),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
}

#[test]
fn long_literals_and_absent_prefixes_have_fixed_cumulative_work_and_allocation_ceilings() {
    let text = "x".repeat(2048) + "É";
    let pattern = "x".repeat(2048) + "é";
    let provider = FrenchLegacyCollation::new(
        order(),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        like(
            &Value::Text(text),
            &Value::Text(pattern),
            Some(LikeMode::Ansi89),
            Some(&provider),
            &mut ResourceBudget::new(1024 * 1024, 0, 100_000),
            None
        ),
        Ok(Truth::True)
    );
    let text = "Les élèves lisent le livre. ".repeat(128);
    let provider = FrenchLegacyCollation::new(
        order(),
        ResourceBudget::new(32 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        like(
            &Value::Text(text),
            &Value::Text("*zèbre*".into()),
            Some(LikeMode::Ansi89),
            Some(&provider),
            &mut ResourceBudget::new(2 * 1024 * 1024, 0, 100_000),
            None
        ),
        Ok(Truth::False)
    );
}
