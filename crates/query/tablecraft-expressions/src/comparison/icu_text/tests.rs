use super::*;
use proptest::prelude::*;
use std::cell::Cell;

fn provider(locale: IcuTextLocale) -> IcuTextCollation<'static> {
    IcuTextCollation::new(
        locale,
        ResourceBudget::new(16_000_000, 0, 10_000_000),
        &|| false,
    )
    .unwrap()
}

#[test]
fn explicit_icu_secondary_contract_keeps_accents_and_locale_tailorings() {
    for locale in [
        IcuTextLocale::EnUs,
        IcuTextLocale::DeDe,
        IcuTextLocale::SvSe,
        IcuTextLocale::TrTr,
    ] {
        let policy = provider(locale);
        assert_eq!(policy.compare_text("ao", "Ao"), Ok(Ordering::Equal));
        assert_eq!(policy.compare_text("as", "às"), Ok(Ordering::Less));
        assert_eq!(policy.compare_text("às", "at"), Ok(Ordering::Less));
        assert_eq!(policy.compare_text("A", "Ⓐ"), Ok(Ordering::Equal));
        assert_eq!(policy.compare_text("é", "e\u{301}"), Ok(Ordering::Equal));
        assert_ne!(policy.compare_text("e", "é"), Ok(Ordering::Equal));
        assert_ne!(policy.compare_text("a-b", "ab"), Ok(Ordering::Equal));
    }
    assert_eq!(
        provider(IcuTextLocale::SvSe).compare_text("z", "ö"),
        Ok(Ordering::Less)
    );
    assert_eq!(
        provider(IcuTextLocale::DeDe).compare_text("ö", "z"),
        Ok(Ordering::Less)
    );
    let turkish = provider(IcuTextLocale::TrTr);
    assert_eq!(turkish.compare_text("I", "ı"), Ok(Ordering::Equal));
    assert_eq!(turkish.compare_text("İ", "i"), Ok(Ordering::Equal));
    assert_ne!(turkish.compare_text("I", "i"), Ok(Ordering::Equal));
}

#[test]
fn searches_return_original_utf16_spans_for_canonical_and_expanded_matches() {
    let policy = provider(IcuTextLocale::EnUs);
    for (haystack, needle, expected) in [
        ("😀Ae\u{301}Z", "É", 3..5),
        ("xⒶy", "a", 1..2),
        ("xxﬁz", "fi", 2..3),
        ("xFIz", "ﬁ", 1..3),
        ("xA\0b", "a", 1..2),
    ] {
        assert_eq!(
            policy.find(
                haystack,
                needle,
                StringSearch::ForwardFrom(0),
                StringCompareMode::Text
            ),
            Ok(Some(expected))
        );
    }
    assert_eq!(
        policy.find(
            "aAa",
            "A",
            StringSearch::ReverseThrough(3),
            StringCompareMode::Text
        ),
        Ok(Some(2..3))
    );
    assert_eq!(
        policy.find(
            "aAa",
            "A",
            StringSearch::ReverseThrough(2),
            StringCompareMode::Text
        ),
        Ok(Some(1..2))
    );
    assert_eq!(
        policy.find(
            "😀A",
            "a",
            StringSearch::ForwardFrom(1),
            StringCompareMode::Text
        ),
        Ok(Some(2..3))
    );
    assert_eq!(
        policy.find(
            "😀A",
            "a",
            StringSearch::ReverseThrough(2),
            StringCompareMode::Text
        ),
        Ok(None)
    );
}

#[test]
fn lcid_registration_is_exact_without_language_or_alternate_sort_aliases() {
    let id = LocaleId::new(0x10409).unwrap();
    let policy = provider(IcuTextLocale::TrTr).with_registered_locale(id);
    assert_eq!(
        policy.find_locale("xI", "ı", StringSearch::ForwardFrom(0), id),
        Ok(Some(1..2))
    );
    for absent in [1033, 1055, 0x20409] {
        assert_eq!(
            policy.find_locale(
                "xI",
                "ı",
                StringSearch::ForwardFrom(0),
                LocaleId::new(absent).unwrap()
            ),
            Err(EvalError::CollationRequired)
        );
    }
    assert_eq!(
        provider(IcuTextLocale::EnUs).find_locale(
            "a",
            "A",
            StringSearch::ForwardFrom(0),
            LocaleId::new(1033).unwrap()
        ),
        Err(EvalError::CollationRequired)
    );
}

struct Database;
impl Collation for Database {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Ok(Ordering::Greater)
    }
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }
    fn compare_utf16(&self, _: &[u16], _: &[u16]) -> Result<Ordering, EvalError> {
        Ok(Ordering::Less)
    }
    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }
    fn find(
        &self,
        _: &str,
        _: &str,
        _: StringSearch,
        _: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        Err(EvalError::CollationFailed)
    }
}

#[test]
fn database_and_raw_profiles_are_not_replaced_by_the_text_provider() {
    let policy = provider(IcuTextLocale::EnUs).with_database(&Database);
    assert_eq!(policy.compare("a", "a"), Ok(Ordering::Greater));
    assert_eq!(policy.compare_text("a", "A"), Ok(Ordering::Equal));
    assert_eq!(policy.option_compare(), Some(StringCompareMode::Database));
    assert_eq!(
        policy.string_comparison_profile(),
        StringComparisonProfile::Utf16
    );
    assert_eq!(policy.compare_utf16(&[0xd800], &[0]), Ok(Ordering::Less));
    assert_eq!(policy.like_profile(), LikeProfile::Utf16Weights);
    assert_eq!(
        policy.find(
            "a",
            "A",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Database
        ),
        Err(EvalError::CollationFailed)
    );
    assert_eq!(
        policy.find_utf16(
            &[0xd800],
            &[0xd800],
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text
        ),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        policy.string_search_profile(StringCompareMode::Text),
        StringSearchProfile::Unicode
    );
    let text_only = provider(IcuTextLocale::EnUs);
    assert_eq!(
        text_only.compare("a", "A"),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(text_only.option_compare(), None);
}

#[test]
fn bounds_quotas_and_constructor_validation_remain_explicit() {
    for (bytes, units) in [(0, 1), (65_537, 1), (1, 0), (1, 4097)] {
        assert_eq!(IcuTextLimits::new(bytes, units), Err(EvalError::Domain));
    }
    let policy = provider(IcuTextLocale::EnUs).with_limits(IcuTextLimits::new(8, 3).unwrap());
    assert_eq!(
        policy.compare_text("123456789", "1"),
        Err(EvalError::TextLimit)
    );
    assert_eq!(
        policy.find(
            "😀😀",
            "a",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text
        ),
        Err(EvalError::TextLimit)
    );
    let policy = IcuTextCollation::new(
        IcuTextLocale::EnUs,
        ResourceBudget::new(1000, 0, 7),
        &|| false,
    )
    .unwrap();
    assert_eq!(policy.compare_text("a", "A"), Ok(Ordering::Equal));
    assert!(matches!(
        policy.compare_text("a", "A"),
        Err(EvalError::Limit(_))
    ));
    let policy = IcuTextCollation::new(
        IcuTextLocale::EnUs,
        ResourceBudget::new(0, 0, 1000),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        policy.find(
            "abc",
            "z",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text
        ),
        Err(EvalError::Limit(_))
    ));
}

#[test]
fn every_search_checkpoint_can_cancel_after_a_successful_prefix() {
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let run = || {
        let policy = IcuTextCollation::new(
            IcuTextLocale::EnUs,
            ResourceBudget::new(100_000, 0, 100_000),
            &cancelled,
        )?;
        policy.find(
            "abca",
            "Z",
            StringSearch::ForwardFrom(0),
            StringCompareMode::Text,
        )
    };
    assert_eq!(run(), Ok(None));
    let total = calls.get();
    for at in 1..=total {
        calls.set(0);
        stop.set(at);
        assert_eq!(run(), Err(EvalError::Cancelled), "checkpoint {at}");
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn ascii_search_agrees_with_independent_case_insensitive_slice_positions(
        haystack in "[a-cA-C]{0,12}", needle in "[a-cA-C]{1,4}", start in 0usize..15, reverse in any::<bool>(),
    ) {
        let lower = haystack.to_ascii_lowercase();
        let needle_lower = needle.to_ascii_lowercase();
        let mut positions = (0..haystack.len()).filter(|offset| {
            lower.get(*offset..offset + needle.len()) == Some(needle_lower.as_str())
        });
        let found = if reverse {
            positions.rfind(|offset| offset + needle.len() <= start)
        } else {
            positions.find(|offset| *offset >= start)
        };
        let expected = found.map(|offset| offset..offset + needle.len());
        let search = if reverse {
            StringSearch::ReverseThrough(start)
        } else {
            StringSearch::ForwardFrom(start)
        };

        prop_assert_eq!(
            provider(IcuTextLocale::EnUs).find(&haystack, &needle, search, StringCompareMode::Text),
            Ok(expected)
        );
    }
}
