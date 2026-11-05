//! Inert typed facts for host display choices; never an edit or query policy.
use jiff::civil::DateTime;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeFormat {
    #[default]
    Unspecified,
    GeneralDate,
    ShortDate,
    MediumDate,
    LongDate,
    ShortTime,
    MediumTime,
    LongTime,
    Currency,
    Euro,
    /// Exact stored `€ #,##0.00;-€ #,##0.00` display pattern.
    EuroPrefixTwoDecimals,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateRepresentation {
    LegacyBits(u64),
    Extended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarPresentation {
    Currency {
        scaled: i64,
        native_format: NativeFormat,
    },
    /// Derived Avg(Currency) retains the query's finite binary64 carrier.
    /// This is neither a stored Currency value nor a four-decimal conversion.
    CurrencyAverage {
        bits: u64,
        native_format: NativeFormat,
    },
    DateTime {
        calendar: DateTime,
        representation: DateRepresentation,
        native_format: NativeFormat,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DateFormat {
    #[default]
    SystemLocale,
    Iso,
    DayMonthYear,
    MonthDayYear,
    Short,
    Long,
    Raw,
}

impl DateFormat {
    pub const ALL: [Self; 7] = [
        Self::SystemLocale,
        Self::Iso,
        Self::DayMonthYear,
        Self::MonthDayYear,
        Self::Short,
        Self::Long,
        Self::Raw,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TimeFormat {
    #[default]
    SystemLocale,
    HoursMinutes,
    HoursMinutesSeconds,
    TwelveHour,
    Fractional,
    Raw,
}

impl TimeFormat {
    pub const ALL: [Self; 6] = [
        Self::SystemLocale,
        Self::HoursMinutes,
        Self::HoursMinutesSeconds,
        Self::TwelveHour,
        Self::Fractional,
        Self::Raw,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CurrencyChoice {
    #[default]
    SystemLocale,
    Raw,
    Usd,
    Eur,
    Gbp,
    Jpy,
    Krw,
    Cny,
    Brl,
    Rub,
    Cad,
    Aud,
    Inr,
    Chf,
    Kwd,
    Bhd,
    Tnd,
}

impl CurrencyChoice {
    pub const ALL: [Self; 17] = [
        Self::SystemLocale,
        Self::Raw,
        Self::Usd,
        Self::Eur,
        Self::Gbp,
        Self::Jpy,
        Self::Krw,
        Self::Cny,
        Self::Brl,
        Self::Rub,
        Self::Cad,
        Self::Aud,
        Self::Inr,
        Self::Chf,
        Self::Kwd,
        Self::Bhd,
        Self::Tnd,
    ];

    pub fn code(self) -> Option<&'static str> {
        match self {
            Self::SystemLocale | Self::Raw => None,
            Self::Usd => Some("USD"),
            Self::Eur => Some("EUR"),
            Self::Gbp => Some("GBP"),
            Self::Jpy => Some("JPY"),
            Self::Krw => Some("KRW"),
            Self::Cny => Some("CNY"),
            Self::Brl => Some("BRL"),
            Self::Rub => Some("RUB"),
            Self::Cad => Some("CAD"),
            Self::Aud => Some("AUD"),
            Self::Inr => Some("INR"),
            Self::Chf => Some("CHF"),
            Self::Kwd => Some("KWD"),
            Self::Bhd => Some("BHD"),
            Self::Tnd => Some("TND"),
        }
    }
}

/// User-facing display policy only; SQL, native bytes and edit inputs are separate.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Options {
    pub system_locale: Option<String>,
    pub date: DateFormat,
    pub time: TimeFormat,
    pub currency: CurrencyChoice,
}
