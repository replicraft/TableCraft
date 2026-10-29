use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};
use tablecraft_access::{FrenchLegacyCollation, GeneralCollation, NativeCollationResolver};
use tablecraft_ace::{InspectError, NativeSortOrder};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{
    EvalError,
    comparison::{
        Collation, LikeProfile, LocaleId, StringCompareMode, StringSearch, StringSearchProfile,
    },
};
use tablecraft_query::{
    QueryError,
    source::{CollationResolver, TextCollationId},
};

fn order() -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([12, 4, 0, 0])
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000)
}

#[test]
fn exact_constructor_and_french_default_resolver_preserve_general_isolation() {
    for raw in [[9, 4, 0, 0], [12, 4, 1, 0], [12, 4, 0, 1], [0, 0, 0, 0]] {
        assert!(matches!(
            FrenchLegacyCollation::new(NativeSortOrder::from_native_bytes(raw), budget(), &|| {
                false
            }),
            Err(InspectError::Unsupported)
        ));
    }
    assert!(matches!(
        GeneralCollation::new(order(), budget(), &|| false),
        Err(InspectError::Unsupported)
    ));
    let provider = FrenchLegacyCollation::new(order(), budget(), &|| false).unwrap();
    assert_eq!(provider.compare("coté", "côte"), Ok(Ordering::Greater));
    assert_eq!(
        provider.compare_utf16(&[0xd800, 65, 32], &[0xd800, 97]),
        Ok(Ordering::Equal)
    );

    let pool = RefCell::new(budget());
    let resolver = NativeCollationResolver::new(order(), &pool, &|| false).unwrap();
    assert_eq!(
        resolver.default_collation().compare("coté", "côte"),
        Ok(Ordering::Greater)
    );
    assert_eq!(
        resolver
            .resolve(TextCollationId::from_host(1036))
            .unwrap()
            .compare("coté", "côte"),
        Ok(Ordering::Greater)
    );
    for key in [1033, 0x01000409] {
        assert_eq!(
            resolver
                .resolve(TextCollationId::from_host(key))
                .unwrap()
                .compare("coté", "côte"),
            Ok(Ordering::Less)
        );
    }
    for key in [0x0100040c, 0x0001040c, 0, u64::MAX] {
        assert!(matches!(
            resolver.resolve(TextCollationId::from_host(key)),
            Err(QueryError::Evaluation(EvalError::CollationRequired))
        ));
    }
}

#[test]
fn database_matching_does_not_supply_text_or_lcid_policies() {
    let provider = FrenchLegacyCollation::new(order(), budget(), &|| false)
        .unwrap()
        .with_option_compare(StringCompareMode::Database);
    assert_eq!(provider.option_compare(), Some(StringCompareMode::Database));
    assert_eq!(provider.like_profile(), LikeProfile::Utf16Weights);
    assert_eq!(
        provider.compare_text("a", "A"),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        provider.string_search_profile(StringCompareMode::Text),
        StringSearchProfile::Unicode
    );
    assert_eq!(
        provider.find(
            "abc",
            "b",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text
        ),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        provider.find_utf16(
            &[97, 98, 99],
            &[98],
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text
        ),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        provider.string_search_profile(StringCompareMode::Database),
        StringSearchProfile::Utf16
    );
    assert_eq!(
        provider.find(
            "cote côte",
            "côte",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database
        ),
        Ok(Some(5..9))
    );
    assert_eq!(
        provider.find_utf16(
            &[0xd800, 0x00e9],
            &[0x00c9],
            StringSearch::ForwardFrom(1),
            StringCompareMode::Database
        ),
        Ok(Some(1..2))
    );
    assert_eq!(
        provider.find_locale(
            "abc",
            "b",
            StringSearch::ForwardFrom(0),
            LocaleId::new(1036).unwrap()
        ),
        Err(EvalError::CollationRequired)
    );
}

#[test]
fn french_search_uses_the_shared_comparison_pool_and_refuses_reentrant_callbacks() {
    let pool = RefCell::new(ResourceBudget::new(4000, 0, 100_000));
    let resolver = NativeCollationResolver::new(order(), &pool, &|| false).unwrap();
    let provider = resolver.default_collation();
    {
        let _held = pool.borrow_mut();
        assert_eq!(
            provider.find(
                "côte",
                "ôte",
                StringSearch::ForwardFrom(0),
                StringCompareMode::Database
            ),
            Err(EvalError::CollationFailed)
        );
        assert_eq!(
            provider.find_utf16(
                &[0x00e9],
                &[0x00e9],
                StringSearch::ForwardFrom(0),
                StringCompareMode::Database
            ),
            Err(EvalError::CollationFailed)
        );
    }
    let mut successes = 0;
    for _ in 0..100 {
        match provider.find(
            "côte",
            "ôte",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database,
        ) {
            Ok(Some(range)) => {
                assert_eq!(range, 1..4);
                successes += 1;
            }
            Err(EvalError::Limit(_)) => break,
            result => panic!("Unexpected search result: {result:?}"),
        }
    }
    assert!(successes > 0 && successes < 100);
    assert!(matches!(
        provider.compare("côte", "côte"),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn french_has_no_equality_hint_and_does_not_activate_for_hint_requests() {
    let cancelled = Cell::new(false);
    let checks = Cell::new(0);
    let callback = || {
        checks.set(checks.get() + 1);
        cancelled.get()
    };
    let pool = RefCell::new(budget());
    let resolver = NativeCollationResolver::new(order(), &pool, &callback).unwrap();
    assert!(
        resolver
            .equality_key(TextCollationId::from_host(1033), "cote")
            .unwrap()
            .is_some()
    );
    let before = checks.get();
    cancelled.set(true);
    let _held = pool.borrow_mut();
    assert!(
        resolver
            .equality_key(TextCollationId::from_host(1036), "coté")
            .unwrap()
            .is_none()
    );
    assert_eq!(checks.get(), before);
}

#[test]
fn shared_comparison_quota_reentrancy_and_reuse_are_preserved() {
    let pool = RefCell::new(ResourceBudget::new(2000, 0, 100_000));
    let resolver = NativeCollationResolver::new(order(), &pool, &|| false).unwrap();
    let french = resolver.resolve(TextCollationId::from_host(1036)).unwrap();
    {
        let _held = pool.borrow_mut();
        assert_eq!(french.compare("a", "b"), Err(EvalError::CollationFailed));
        assert_eq!(
            french.compare_utf16(&[97], &[98]),
            Err(EvalError::CollationFailed)
        );
    }
    let mut successes = 0;
    for _ in 0..100 {
        match french.compare("coté", "côte") {
            Ok(Ordering::Greater) => successes += 1,
            Err(EvalError::Limit(_)) => break,
            other => panic!("Unexpected comparison outcome: {other:?}"),
        }
    }
    assert!(successes > 0 && successes < 100);
    for key in [1036, 1033, 0x01000409] {
        assert!(matches!(
            resolver
                .resolve(TextCollationId::from_host(key))
                .unwrap()
                .compare("cote", "COTE"),
            Err(EvalError::Limit(_))
        ));
    }
}

#[test]
fn constructor_and_every_observed_provider_checkpoint_can_cancel() {
    assert!(matches!(
        FrenchLegacyCollation::new(order(), budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    let provider = FrenchLegacyCollation::new(order(), budget(), &observe).unwrap();
    assert_eq!(provider.compare("coté", "côte"), Ok(Ordering::Greater));
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancelled = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        match FrenchLegacyCollation::new(order(), budget(), &cancelled) {
            Err(InspectError::Cancelled) => assert_eq!(stop, 1),
            Ok(provider) => assert_eq!(provider.compare("coté", "côte"), Err(EvalError::Cancelled)),
            _ => panic!("Unexpected constructor outcome"),
        }
    }
    let pool = RefCell::new(budget());
    assert!(matches!(
        NativeCollationResolver::new(order(), &pool, &|| true),
        Err(InspectError::Cancelled)
    ));
}
