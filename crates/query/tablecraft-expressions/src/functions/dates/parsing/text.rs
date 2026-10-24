//! Validate date and time before either component is discarded by a caller.
use super::{AUTOMATION_EPOCH, DateParseContext, DateTextFormat, date, time};
use crate::EvalError;
use jiff::civil::{Date, DateTime, Time};

pub(super) fn parse(value: &str, context: DateParseContext) -> Result<DateTime, EvalError> {
    parse_with(value, context.date_format, true, |part| {
        date::parse(part, context)
    })
}

pub(super) fn parse_with(
    value: &str,
    format: Option<DateTextFormat>,
    allow_dotted_time: bool,
    parse_date: impl FnOnce(&str) -> Result<Date, EvalError>,
) -> Result<DateTime, EvalError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(EvalError::TypeMismatch);
    }
    let marker = value
        .find(':')
        .or_else(|| {
            (allow_dotted_time && format == Some(DateTextFormat::EnUs))
                .then(|| time::dotted_marker(value))
                .flatten()
        })
        .or_else(|| time::meridiem(value).map(|_| value.len() - time::MERIDIEM_BYTES));
    if let Some(marker) = marker {
        let prefix = value[..marker].trim_end();
        let hour_start = prefix
            .char_indices()
            .rev()
            .find(|(_, ch)| !ch.is_ascii_digit())
            .map_or(0, |(index, ch)| index + ch.len_utf8());
        let date_prefix = &value[..hour_start];
        let time_part = value[hour_start..].trim();
        let date_part = if date_prefix.ends_with('T') && format == Some(DateTextFormat::Iso) {
            date_prefix[..date_prefix.len() - 1].trim_end()
        } else if date_prefix.is_empty() || date_prefix.ends_with(char::is_whitespace) {
            date_prefix.trim_end()
        } else {
            return Err(EvalError::TypeMismatch);
        };
        let time = time::parse(time_part)?;
        let date = if date_part.is_empty() {
            AUTOMATION_EPOCH
        } else {
            parse_date(date_part)?
        };
        return Ok(date.to_datetime(time));
    }
    parse_date(value).map(|date| date.to_datetime(Time::MIN))
}
