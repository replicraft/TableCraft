use super::engine::{Engine, raw_date};
use super::*;
use jiff::civil::date;

fn options(locale: &str) -> Options {
    Options {
        system_locale: Some(locale.to_owned()),
        ..Options::default()
    }
}

fn money(scaled: i64) -> ScalarPresentation {
    ScalarPresentation::Currency {
        scaled,
        native_format: NativeFormat::Currency,
    }
}

#[test]
fn currency_uses_exact_scaled_integer_and_currency_minor_units() {
    let mut policy = options("en-US");
    for (choice, expected) in [
        (CurrencyChoice::Usd, "$1,234.57"),
        (CurrencyChoice::Jpy, "¥1,235"),
        (CurrencyChoice::Kwd, "KWD\u{a0}1,234.568"),
    ] {
        policy.currency = choice;
        let engine = Engine::new(&policy).unwrap();
        assert_eq!(engine.format(money(12_345_678)), expected);
    }

    policy.currency = CurrencyChoice::Raw;
    let engine = Engine::new(&policy).unwrap();
    assert_eq!(engine.format(money(i64::MIN)), "-922337203685477.5808");
    assert_eq!(engine.format(money(i64::MAX)), "922337203685477.5807");
    assert_eq!(engine.format(money(1)), "0.0001");
}

#[test]
fn dates_preserve_regional_order_full_year_and_independent_time_choices() {
    let calendar = date(2026, 4, 12).at(15, 4, 5, 123_456_700);
    for (locale, expected) in [("en-US", "4/12/2026"), ("de-DE", "12.04.2026")] {
        let mut policy = options(locale);
        policy.time = TimeFormat::HoursMinutesSeconds;
        let engine = Engine::new(&policy).unwrap();
        let date_only = ScalarPresentation::DateTime {
            calendar,
            representation: DateRepresentation::Extended,
            native_format: NativeFormat::ShortDate,
        };
        assert_eq!(engine.format(date_only), expected);
        let time_only = ScalarPresentation::DateTime {
            calendar: date(1899, 12, 30).at(15, 4, 5, 123_456_700),
            representation: DateRepresentation::Extended,
            native_format: NativeFormat::ShortTime,
        };
        assert_eq!(engine.format(time_only), "15:04:05");
    }

    assert_eq!(
        raw_date(calendar, DateRepresentation::Extended),
        "DATETIME2(2026-04-12T15:04:05.1234567)"
    );
    assert_eq!(
        raw_date(calendar, DateRepresentation::LegacyBits(0x8000000000000000)),
        "DATE(0x8000000000000000)"
    );
}

#[test]
fn locale_currency_selection_is_explicit_for_ambiguous_regions() {
    for (locale, expected) in [
        ("en-US", Some("USD")),
        ("de-DE", Some("EUR")),
        ("ja-JP", Some("JPY")),
        ("pt-BR", Some("BRL")),
        ("en-IO", None),
    ] {
        let engine = Engine::new(&options(locale)).unwrap();
        assert_eq!(engine.system_currency.as_deref(), expected, "{locale}");
    }

    let engine = Engine::new(&Options::default()).unwrap();
    assert!(engine.fallback_locale);
    assert_eq!(engine.locale, "en-US");
}

#[test]
fn general_date_omits_absent_components_and_raw_retains_full_native_carrier() {
    let mut policy = options("de-DE");
    let engine = Engine::new(&policy).unwrap();
    for native_format in [NativeFormat::Unspecified, NativeFormat::GeneralDate] {
        for (calendar, expected) in [
            (date(2026, 4, 12).at(0, 0, 0, 0), "12.04.2026"),
            (date(1899, 12, 30).at(18, 0, 0, 0), "18:00:00"),
            (date(2026, 4, 12).at(15, 4, 5, 0), "12.04.2026 15:04:05"),
        ] {
            assert_eq!(
                engine.format(ScalarPresentation::DateTime {
                    calendar,
                    representation: DateRepresentation::Extended,
                    native_format,
                }),
                expected
            );
        }
    }
    policy.time = TimeFormat::Raw;
    let engine = Engine::new(&policy).unwrap();
    assert_eq!(
        engine.format(ScalarPresentation::DateTime {
            calendar: date(1899, 12, 30).at(15, 4, 5, 123_456_700),
            representation: DateRepresentation::Extended,
            native_format: NativeFormat::Unspecified,
        }),
        "DATETIME2(1899-12-30T15:04:05.1234567)"
    );
}

#[test]
fn exact_stored_euro_prefix_overrides_system_currency_without_float_rounding() {
    for (locale, positive, negative) in [
        ("en-US", "€ 1,234.57", "-€ 1,234.57"),
        ("de-DE", "€ 1.234,57", "-€ 1.234,57"),
    ] {
        let engine = Engine::new(&options(locale)).unwrap();
        for (scaled, expected) in [(12_345_650, positive), (-12_345_650, negative)] {
            assert_eq!(
                engine.format(ScalarPresentation::Currency {
                    scaled,
                    native_format: NativeFormat::EuroPrefixTwoDecimals,
                }),
                expected
            );
        }
    }
}

#[test]
fn option_examples_use_captured_region_and_the_same_typed_formatter_as_cells() {
    let calendar = date(2026, 4, 12).at(15, 4, 5, 123_456_700);
    for locale in ["en-US", "de-DE", "ja-JP"] {
        let policy = options(locale);
        let examples = Engine::new(&policy).unwrap();
        for choice in DateFormat::ALL {
            let mut selected = policy.clone();
            selected.date = choice;
            let engine = Engine::new(&selected).unwrap();
            let fact = ScalarPresentation::DateTime {
                calendar,
                representation: DateRepresentation::Extended,
                native_format: NativeFormat::ShortDate,
            };
            assert_eq!(examples.date_example(choice), engine.format(fact));
        }
        for choice in TimeFormat::ALL {
            let mut selected = policy.clone();
            selected.time = choice;
            let engine = Engine::new(&selected).unwrap();
            let fact = ScalarPresentation::DateTime {
                calendar,
                representation: DateRepresentation::Extended,
                native_format: NativeFormat::LongTime,
            };
            assert_eq!(examples.time_example(choice), engine.format(fact));
        }
    }

    let engine = Engine::new(&options("en-US")).unwrap();
    assert_eq!(engine.date_example(DateFormat::DayMonthYear), "12/04/2026");
    assert_eq!(engine.date_example(DateFormat::MonthDayYear), "04/12/2026");
    assert_eq!(engine.time_example(TimeFormat::HoursMinutes), "15:04");
    assert_eq!(
        engine.time_example(TimeFormat::Fractional),
        "15:04:05.1234567"
    );
    for (currency, expected) in [
        (CurrencyChoice::Usd, "$1,234.57"),
        (CurrencyChoice::Jpy, "¥1,235"),
        (CurrencyChoice::Kwd, "KWD\u{a0}1,234.568"),
        (CurrencyChoice::Raw, "1234.5678"),
    ] {
        let mut policy = options("en-US");
        policy.currency = currency;
        assert_eq!(Engine::new(&policy).unwrap().currency_example(), expected);
    }
}
