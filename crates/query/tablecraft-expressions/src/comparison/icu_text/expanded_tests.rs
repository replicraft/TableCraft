use super::*;

fn provider(locale: IcuTextLocale) -> IcuTextCollation<'static> {
    IcuTextCollation::new(
        locale,
        ResourceBudget::new(1_000_000, 0, 1_000_000),
        &|| false,
    )
    .unwrap()
}

#[test]
fn every_explicit_profile_round_trips_without_accepting_extensions_or_aliases() {
    assert_eq!(IcuTextLocale::ALL.len(), 19);
    for (index, &locale) in IcuTextLocale::ALL.iter().enumerate() {
        assert_eq!(IcuTextLocale::from_name(locale.as_str()), Some(locale));
        assert_eq!(
            IcuTextLocale::from_name(&locale.as_str().to_ascii_uppercase()),
            Some(locale)
        );
        for &before in &IcuTextLocale::ALL[..index] {
            assert_ne!(before.as_str(), locale.as_str());
        }

        let policy = provider(locale);
        assert_eq!(policy.compare_text("A", "a"), Ok(Ordering::Equal));
        assert_eq!(policy.compare_text("é", "e\u{301}"), Ok(Ordering::Equal));
        assert_ne!(policy.compare_text("e", "é"), Ok(Ordering::Equal));
    }
    for invalid in [
        "",
        "en",
        "en_US",
        " en-US",
        "fr-FR ",
        "zz-ZZ",
        "zh-Hans-CN",
        "en-US-u-co-search",
        "de-DE-u-co-phonebk",
        "es-ES-u-co-trad",
    ] {
        assert_eq!(IcuTextLocale::from_name(invalid), None, "{invalid}");
    }
}

#[test]
fn added_profiles_keep_independently_observed_language_tailorings() {
    for (locale, left, right, expected) in [
        (IcuTextLocale::FrFr, "cote", "côte", Ordering::Less),
        (IcuTextLocale::EsEs, "n", "ñ", Ordering::Less),
        (IcuTextLocale::EsEs, "ñ", "o", Ordering::Less),
        (IcuTextLocale::EsEs, "chico", "cielo", Ordering::Less),
        (IcuTextLocale::ItIt, "casa", "cosa", Ordering::Less),
        (IcuTextLocale::NlNl, "ij", "y", Ordering::Less),
        (IcuTextLocale::PlPl, "l", "ł", Ordering::Less),
        (IcuTextLocale::PlPl, "ł", "m", Ordering::Less),
        (IcuTextLocale::CsCz, "h", "ch", Ordering::Less),
        (IcuTextLocale::CsCz, "ch", "i", Ordering::Less),
        (IcuTextLocale::JaJp, "あ", "ア", Ordering::Equal),
        (IcuTextLocale::JaJp, "あ", "い", Ordering::Less),
        (IcuTextLocale::ZhCn, "阿", "八", Ordering::Less),
        (IcuTextLocale::ZhTw, "阿", "八", Ordering::Greater),
        (IcuTextLocale::RuRu, "е", "ё", Ordering::Less),
        (IcuTextLocale::RuRu, "е", "ж", Ordering::Less),
        (IcuTextLocale::ElGr, "σ", "ς", Ordering::Equal),
        (IcuTextLocale::ArSa, "ا", "ب", Ordering::Less),
        (IcuTextLocale::HeIl, "א", "ב", Ordering::Less),
        (IcuTextLocale::KoKr, "가", "나", Ordering::Less),
        (IcuTextLocale::PtBr, "a", "ã", Ordering::Less),
    ] {
        assert_eq!(provider(locale).compare_text(left, right), Ok(expected));
    }
}

#[test]
fn added_profile_searches_preserve_original_spans_and_exact_host_registration() {
    let registered = LocaleId::new(0x10409).unwrap();
    let absent = LocaleId::new(0x20409).unwrap();
    for (locale, haystack, needle, expected) in [
        (IcuTextLocale::CsCz, "😀Chx", "ch", 2..4),
        (IcuTextLocale::PlPl, "xŁz", "ł", 1..2),
        (IcuTextLocale::JaJp, "xアz", "あ", 1..2),
        (IcuTextLocale::ZhCn, "x阿z", "阿", 1..2),
        (IcuTextLocale::ElGr, "xςz", "σ", 1..2),
        (IcuTextLocale::HeIl, "xאבz", "א", 1..2),
    ] {
        let policy = provider(locale).with_registered_locale(registered);
        assert_eq!(
            policy.find(
                haystack,
                needle,
                StringSearch::ForwardFrom(0),
                StringCompareMode::Text
            ),
            Ok(Some(expected.clone()))
        );
        assert_eq!(
            policy.find_locale(haystack, needle, StringSearch::ForwardFrom(0), registered),
            Ok(Some(expected))
        );
        assert_eq!(
            policy.find_locale(haystack, needle, StringSearch::ForwardFrom(0), absent),
            Err(EvalError::CollationRequired)
        );
        assert_eq!(policy.compare("A", "a"), Err(EvalError::CollationRequired));
    }
}
