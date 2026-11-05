//! ICU/CLDR text from validated typed facts, without parsing rendered cell text.
use super::model::*;
use icu_calendar::{Date, Gregorian};
use icu_datetime::{
    DateTimeFormatterPreferences, FixedCalendarDateTimeFormatter, NoCalendarFormatter,
    fieldsets::{T, YMD},
    input::Time,
    options::YearStyle,
    preferences::HourCycle,
};
use icu_decimal::input::Decimal;
use icu_experimental::dimension::currency::{CurrencyType, formatter::CurrencyFormatter};
use icu_locale::LocaleExpander;
use icu_locale_core::Locale;
use jiff::civil::DateTime;
use std::collections::HashMap;
use writeable::Writeable;

const CURRENCY_SCALE: i16 = -4;
const NANOS_PER_EXTENDED_TICK: i32 = 100;
const ACCESS_TIME_ONLY_DATE: jiff::civil::Date = jiff::civil::date(1899, 12, 30);

#[derive(Debug, thiserror::Error)]
#[error("locale display data could not be prepared")]
pub(crate) struct FormatError;

pub(crate) struct Engine {
    options: Options,
    pub locale: String,
    pub fallback_locale: bool,
    pub system_currency: Option<String>,
    date_short: FixedCalendarDateTimeFormatter<Gregorian, YMD>,
    date_medium: FixedCalendarDateTimeFormatter<Gregorian, YMD>,
    date_long: FixedCalendarDateTimeFormatter<Gregorian, YMD>,
    time_medium: NoCalendarFormatter<T>,
    time_twelve_short: NoCalendarFormatter<T>,
    time_twelve: NoCalendarFormatter<T>,
    euro_number: CurrencyFormatter<icu_decimal::DecimalFormatter>,
    currencies: HashMap<String, CurrencyFormatter<icu_decimal::DecimalFormatter>>,
}

impl Engine {
    pub fn new(options: &Options) -> Result<Self, FormatError> {
        let parsed = options
            .system_locale
            .as_deref()
            .and_then(|name| name.parse::<Locale>().ok());
        let fallback_locale = parsed.is_none();
        let locale = parsed.unwrap_or(icu_locale_core::locale!("en-US"));
        let mut expanded = locale.id.clone();
        LocaleExpander::new_extended().maximize(&mut expanded);
        let system_currency = expanded.region.and_then(|region| {
            let country = region.to_string().parse::<iso_currency::Country>().ok()?;
            regional_currency(country)
        });
        let preferences: DateTimeFormatterPreferences = locale.clone().into();
        let mut twelve = preferences;
        twelve.hour_cycle = Some(HourCycle::H12);
        let mut currencies = HashMap::new();
        let selected_code = match options.currency {
            CurrencyChoice::SystemLocale => system_currency.as_deref(),
            explicit => explicit.code(),
        };
        // Prepare only formats that this display policy can actually consume.
        for code in selected_code.into_iter().chain(Some("EUR")) {
            if currencies.contains_key(code) {
                continue;
            }
            let currency: CurrencyType = code.parse().map_err(|_| FormatError)?;
            let formatter = CurrencyFormatter::try_new_symbol(
                locale.clone().into(),
                currency,
                Default::default(),
            )
            .map_err(|_| FormatError)?;
            currencies.insert(code.to_owned(), formatter);
        }
        let euro_number = CurrencyFormatter::try_new_no_currency(
            locale.clone().into(),
            "EUR".parse().map_err(|_| FormatError)?,
            Default::default(),
        )
        .map_err(|_| FormatError)?;

        Ok(Self {
            options: options.clone(),
            locale: locale.to_string(),
            fallback_locale,
            system_currency,
            date_short: FixedCalendarDateTimeFormatter::try_new(
                preferences,
                YMD::short().with_year_style(YearStyle::Full),
            )
            .map_err(|_| FormatError)?,
            date_medium: FixedCalendarDateTimeFormatter::try_new(preferences, YMD::medium())
                .map_err(|_| FormatError)?,
            date_long: FixedCalendarDateTimeFormatter::try_new(preferences, YMD::long())
                .map_err(|_| FormatError)?,
            time_medium: NoCalendarFormatter::try_new(preferences, T::medium())
                .map_err(|_| FormatError)?,
            time_twelve_short: NoCalendarFormatter::try_new(twelve, T::short())
                .map_err(|_| FormatError)?,
            time_twelve: NoCalendarFormatter::try_new(twelve, T::medium())
                .map_err(|_| FormatError)?,
            euro_number,
            currencies,
        })
    }

    pub fn system_examples(&self) -> (String, String) {
        // Stable illustrative values, independent of the wall clock or database.
        let date = Date::try_new_gregorian(2026, 4, 12).expect("literal example date");
        let time = Time::try_new(15, 4, 5, 0).expect("literal example time");
        (
            self.date_short.format(&date).write_to_string().into_owned(),
            self.time_medium
                .format(&time)
                .write_to_string()
                .into_owned(),
        )
    }

    pub fn format(&self, presentation: ScalarPresentation) -> String {
        match presentation {
            ScalarPresentation::Currency {
                scaled,
                native_format,
            } => self.currency(scaled, native_format),
            ScalarPresentation::DateTime {
                calendar,
                representation,
                native_format,
            } => self.datetime(calendar, representation, native_format),
        }
    }

    fn currency(&self, scaled: i64, native_format: NativeFormat) -> String {
        let mut amount = Decimal::from(scaled);
        amount.multiply_pow10(CURRENCY_SCALE);
        if self.options.currency == CurrencyChoice::SystemLocale
            && native_format == NativeFormat::EuroPrefixTwoDecimals
        {
            let mut magnitude = Decimal::from(scaled.unsigned_abs());
            magnitude.multiply_pow10(CURRENCY_SCALE);
            let number = self.euro_number.format_fixed_decimal(&magnitude);
            let sign = if scaled < 0 { "-" } else { "" };
            return format!("{sign}€ {number}");
        }

        let code = match self.options.currency {
            CurrencyChoice::Raw => return amount.to_string(),
            CurrencyChoice::SystemLocale if native_format == NativeFormat::Euro => Some("EUR"),
            CurrencyChoice::SystemLocale if native_format == NativeFormat::Unsupported => {
                return amount.to_string();
            }
            CurrencyChoice::SystemLocale => self.system_currency.as_deref(),
            explicit => explicit.code(),
        };
        code.and_then(|code| self.currencies.get(code)).map_or_else(
            || amount.to_string(),
            |formatter| {
                formatter
                    .format_fixed_decimal(&amount)
                    .write_to_string()
                    .into_owned()
            },
        )
    }

    fn datetime(
        &self,
        calendar: DateTime,
        representation: DateRepresentation,
        native_format: NativeFormat,
    ) -> String {
        let general_date = matches!(
            native_format,
            NativeFormat::Unspecified | NativeFormat::GeneralDate
        );
        // Access General Date omits absent components. This is native typed
        // DATE behavior, not a guess based on the field name or rendered text.
        let no_time = calendar.time() == jiff::civil::Time::midnight();
        let no_date = calendar.date() == ACCESS_TIME_ONLY_DATE;
        let date_only = (general_date && no_time)
            || matches!(
                native_format,
                NativeFormat::ShortDate | NativeFormat::MediumDate | NativeFormat::LongDate
            );
        let time_only = (general_date && no_date && !no_time)
            || matches!(
                native_format,
                NativeFormat::ShortTime | NativeFormat::MediumTime | NativeFormat::LongTime
            );
        let raw = (!time_only && self.options.date == DateFormat::Raw)
            || (!date_only && self.options.time == TimeFormat::Raw)
            || (native_format == NativeFormat::Unsupported
                && self.options.date == DateFormat::SystemLocale
                && self.options.time == TimeFormat::SystemLocale);
        if raw {
            return raw_date(calendar, representation);
        }
        let date = if time_only {
            None
        } else {
            Some(self.date(calendar, native_format))
        };
        let time = if date_only {
            None
        } else {
            Some(self.time(calendar, representation, native_format))
        };
        match (date, time) {
            (Some(date), Some(time)) => format!("{date} {time}"),
            (Some(date), None) => date,
            (None, Some(time)) => time,
            (None, None) => raw_date(calendar, representation),
        }
    }

    fn date(&self, calendar: DateTime, native_format: NativeFormat) -> String {
        let value = Date::try_new_gregorian(
            i32::from(calendar.year()),
            calendar.month() as u8,
            calendar.day() as u8,
        );
        let Ok(value) = value else {
            return calendar.date().to_string();
        };
        match self.options.date {
            DateFormat::Iso => calendar.date().to_string(),
            DateFormat::DayMonthYear => calendar.strftime("%d/%m/%Y").to_string(),
            DateFormat::MonthDayYear => calendar.strftime("%m/%d/%Y").to_string(),
            DateFormat::Long => self.date_long.format(&value).write_to_string().into_owned(),
            DateFormat::SystemLocale if native_format == NativeFormat::LongDate => {
                self.date_long.format(&value).write_to_string().into_owned()
            }
            DateFormat::SystemLocale if native_format == NativeFormat::MediumDate => self
                .date_medium
                .format(&value)
                .write_to_string()
                .into_owned(),
            _ => self
                .date_short
                .format(&value)
                .write_to_string()
                .into_owned(),
        }
    }

    fn time(
        &self,
        calendar: DateTime,
        representation: DateRepresentation,
        native_format: NativeFormat,
    ) -> String {
        let value = Time::try_new(
            calendar.hour() as u8,
            calendar.minute() as u8,
            calendar.second() as u8,
            calendar.subsec_nanosecond() as u32,
        );
        let Ok(value) = value else {
            return calendar.time().to_string();
        };
        match self.options.time {
            TimeFormat::HoursMinutes => calendar.strftime("%H:%M").to_string(),
            TimeFormat::HoursMinutesSeconds => calendar.strftime("%H:%M:%S").to_string(),
            TimeFormat::TwelveHour => self
                .time_twelve
                .format(&value)
                .write_to_string()
                .into_owned(),
            TimeFormat::Fractional => match representation {
                DateRepresentation::Extended => format!(
                    "{}.{:07}",
                    calendar.strftime("%H:%M:%S"),
                    calendar.subsec_nanosecond() / NANOS_PER_EXTENDED_TICK
                ),
                DateRepresentation::LegacyBits(_) => format!(
                    "{}.{:09}",
                    calendar.strftime("%H:%M:%S"),
                    calendar.subsec_nanosecond()
                ),
            },
            TimeFormat::SystemLocale if native_format == NativeFormat::ShortTime => {
                calendar.strftime("%H:%M").to_string()
            }
            TimeFormat::SystemLocale if native_format == NativeFormat::MediumTime => self
                .time_twelve_short
                .format(&value)
                .write_to_string()
                .into_owned(),
            TimeFormat::SystemLocale if native_format == NativeFormat::LongTime => self
                .time_medium
                .format(&value)
                .write_to_string()
                .into_owned(),
            _ => self
                .time_medium
                .format(&value)
                .write_to_string()
                .into_owned(),
        }
    }
}

pub(crate) fn raw(presentation: ScalarPresentation) -> String {
    match presentation {
        ScalarPresentation::Currency { scaled, .. } => {
            let mut amount = Decimal::from(scaled);
            amount.multiply_pow10(CURRENCY_SCALE);
            amount.to_string()
        }
        ScalarPresentation::DateTime {
            calendar,
            representation,
            ..
        } => raw_date(calendar, representation),
    }
}

pub(crate) fn raw_date(calendar: DateTime, representation: DateRepresentation) -> String {
    match representation {
        DateRepresentation::LegacyBits(bits) => format!("DATE(0x{bits:016x})"),
        DateRepresentation::Extended => format!(
            "DATETIME2({}.{:07})",
            calendar.strftime("%Y-%m-%dT%H:%M:%S"),
            calendar.subsec_nanosecond() / NANOS_PER_EXTENDED_TICK
        ),
    }
}

// Regions with multiple regular currencies require an explicit display choice.
// Never infer a stored unit from whichever ISO entry happens to come first.
fn regional_currency(country: iso_currency::Country) -> Option<String> {
    let currencies = iso_currency::Currency::from_country(country);
    let mut regular = currencies
        .iter()
        .filter(|currency| currency.flags().is_empty());
    let first = regular.next()?;
    if regular.next().is_some() {
        return None;
    }

    Some(first.code().to_owned())
}
