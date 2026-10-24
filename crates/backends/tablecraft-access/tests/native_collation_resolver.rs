use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};
use tablecraft_access::{GeneralCollation, NativeCollationResolver};
use tablecraft_ace::{InspectError, NativeSortOrder};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
    predicates::{LikeMode, like},
};
use tablecraft_query::{
    QueryError,
    source::{CollationResolver, TextCollationId},
};

fn order(version: u8) -> NativeSortOrder {
    NativeSortOrder::from_native_bytes([9, 4, 0, version])
}
fn id(version: u8) -> TextCollationId {
    TextCollationId::from_host(if version == 0 { 1033 } else { 0x01000409 })
}
fn pool() -> RefCell<ResourceBudget> {
    RefCell::new(ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000))
}
#[test]
fn exact_native_families_and_defaults_resolve_without_guessing_or_fallback() {
    for version in [0, 1] {
        let budget = pool();
        let resolver = NativeCollationResolver::new(order(version), &budget, &|| false).unwrap();
        assert_eq!(resolver.default_collation().option_compare(), None);
        for family in [0, 1] {
            let policy = resolver.resolve(id(family)).unwrap();
            // Independently observed complete short-key order, also replayed by
            // the original Java oracle. This is not native Access SQL evidence.
            assert_eq!(
                policy.compare("\u{80}", "\u{301}").unwrap(),
                if family == 0 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            );
        }
        assert_eq!(
            resolver
                .default_collation()
                .compare("\u{80}", "\u{301}")
                .unwrap(),
            if version == 0 {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        );
        for key in [0, 0x01010409, 0x02000409, 0x0100041f, u64::MAX] {
            assert!(matches!(
                resolver.resolve(TextCollationId::from_host(key)),
                Err(QueryError::Evaluation(EvalError::CollationRequired))
            ));
        }
        let resolver = resolver.with_option_compare(StringCompareMode::Database);
        assert_eq!(
            resolver.default_collation().option_compare(),
            Some(StringCompareMode::Database)
        );
    }
    let budget = pool();
    for raw in [[0, 0, 0, 0], [9, 4, 1, 1], [9, 4, 0, 2], [31, 4, 0, 1]] {
        assert!(matches!(
            NativeCollationResolver::new(NativeSortOrder::from_native_bytes(raw), &budget, &|| {
                false
            }),
            Err(InspectError::Unsupported)
        ));
    }
}
#[test]
fn construction_lookup_and_all_families_share_one_cumulative_step_allowance() {
    let budget = RefCell::new(ResourceBudget::new(1_000_000, 0, 4));
    let resolver = NativeCollationResolver::new(order(0), &budget, &|| false).unwrap();
    let legacy = resolver.resolve(id(0)).unwrap();
    let modern = resolver.resolve(id(1)).unwrap();
    for policy in [legacy, modern, resolver.default_collation()] {
        assert!(matches!(policy.compare("a", "A"), Err(EvalError::Limit(_))));
    }
    assert!(matches!(resolver.resolve(id(0)), Err(QueryError::Limit(_))));
}
#[test]
fn normalization_search_and_like_share_bytes_across_policies_and_reuse() {
    let budget = RefCell::new(ResourceBudget::new(1200, 0, 1_000_000));
    let resolver = NativeCollationResolver::new(order(1), &budget, &|| false).unwrap();
    let legacy = resolver.resolve(id(0)).unwrap();
    let modern = resolver.resolve(id(1)).unwrap();
    let mut successes = 0;
    for number in 0..100 {
        let policy = if number % 2 == 0 { legacy } else { modern };
        match policy.compare("ss", "ß") {
            Ok(Ordering::Equal) => successes += 1,
            Err(EvalError::Limit(_)) => break,
            other => panic!("{other:?}"),
        }
    }
    assert!(successes > 1 && successes < 100);
    for policy in [legacy, modern] {
        assert!(matches!(policy.compare("a", "A"), Err(EvalError::Limit(_))));
        assert!(matches!(
            policy.find(
                "Straße",
                "ss",
                StringSearch::ForwardFrom(0),
                StringCompareMode::Database
            ),
            Err(EvalError::Limit(_))
        ));
        assert!(matches!(
            like(
                &Value::Text("ss".into()),
                &Value::Text("[ß]".into()),
                Some(LikeMode::Ansi89),
                Some(policy),
                &mut ResourceBudget::new(10000, 0, 10000),
                None
            ),
            Err(EvalError::Limit(_))
        ));
    }
}
#[test]
fn borrowed_quota_reentrancy_and_cancellation_fail_explicitly() {
    let budget = pool();
    let cancelled = Cell::new(false);
    let callback = || cancelled.get();
    let resolver = NativeCollationResolver::new(order(0), &budget, &callback).unwrap();
    {
        let _borrow = budget.borrow_mut();
        assert!(matches!(
            resolver.resolve(id(0)),
            Err(QueryError::Evaluation(EvalError::CollationFailed))
        ));
        assert_eq!(
            resolver.default_collation().compare("a", "A"),
            Err(EvalError::CollationFailed)
        );
        assert!(matches!(
            NativeCollationResolver::new(order(0), &budget, &callback),
            Err(InspectError::Unsupported)
        ));
    }
    cancelled.set(true);
    assert!(matches!(
        resolver.resolve(id(0)),
        Err(QueryError::Evaluation(EvalError::Cancelled))
    ));
    assert_eq!(
        resolver.default_collation().compare("a", "A"),
        Err(EvalError::Cancelled)
    );
    assert!(matches!(
        NativeCollationResolver::new(order(0), &budget, &callback),
        Err(InspectError::Cancelled)
    ));
    let standalone =
        GeneralCollation::new(order(0), ResourceBudget::new(1000, 0, 1000), &|| false).unwrap();
    assert_eq!(standalone.option_compare(), None);
    assert_eq!(
        standalone
            .with_option_compare(StringCompareMode::Binary)
            .option_compare(),
        Some(StringCompareMode::Binary)
    );
}

#[test]
fn each_observed_construction_and_resolution_checkpoint_can_cancel() {
    let visits = Cell::new(0usize);
    let stop = Cell::new(None);
    let callback = || {
        let n = visits.get() + 1;
        visits.set(n);
        stop.get() == Some(n)
    };
    let budget = pool();
    NativeCollationResolver::new(order(0), &budget, &callback).unwrap();
    let count = visits.get();
    assert!(count > 1);
    for point in 1..=count {
        let budget = pool();
        visits.set(0);
        stop.set(Some(point));
        assert!(
            matches!(
                NativeCollationResolver::new(order(0), &budget, &callback),
                Err(InspectError::Cancelled)
            ),
            "constructor {point}"
        );
    }
    stop.set(None);
    let resolver = NativeCollationResolver::new(order(0), &budget, &callback).unwrap();
    visits.set(0);
    resolver.resolve(id(0)).unwrap();
    let count = visits.get();
    assert!(count > 1);
    for point in 1..=count {
        visits.set(0);
        stop.set(Some(point));
        assert!(
            matches!(
                resolver.resolve(id(1)),
                Err(QueryError::Evaluation(EvalError::Cancelled))
            ),
            "lookup {point}"
        );
    }
}

#[test]
fn equality_keys_keep_independent_primary_goldens_and_decline_unknown_or_long_literals() {
    let budget = pool();
    let resolver = NativeCollationResolver::new(order(1), &budget, &|| false).unwrap();
    for (family, expected) in [(0, vec![0x4a]), (1, vec![0x0e, 0x02])] {
        let key = resolver.equality_key(id(family), "A  ").unwrap().unwrap();
        assert_eq!(key.policy(), id(family));
        assert_eq!(key.as_bytes(), expected);
    }
    for literal in ["x".repeat(256), "😀".repeat(128)] {
        assert_eq!(resolver.equality_key(id(1), &literal).unwrap(), None);
    }
    assert_eq!(
        resolver
            .equality_key(TextCollationId::from_host(u64::MAX), "a")
            .unwrap(),
        None
    );
}

#[test]
fn equality_key_work_shares_provider_quotas_cancellation_and_borrow_refusals() {
    let budget = RefCell::new(ResourceBudget::new(0, 0, 10000));
    let resolver = NativeCollationResolver::new(order(1), &budget, &|| false).unwrap();
    assert!(matches!(
        resolver.equality_key(id(1), "a"),
        Err(QueryError::Evaluation(EvalError::Limit(_)))
    ));

    let budget = pool();
    let cancelled = Cell::new(false);
    let cancellation = || cancelled.get();
    let resolver = NativeCollationResolver::new(order(1), &budget, &cancellation).unwrap();
    let locked = budget.borrow_mut();
    assert!(matches!(
        resolver.equality_key(id(1), "a"),
        Err(QueryError::Evaluation(EvalError::CollationFailed))
    ));
    cancelled.set(true);
    assert!(matches!(
        resolver.equality_key(id(1), "a"),
        Err(QueryError::Evaluation(EvalError::Cancelled))
    ));
    drop(locked);
}
