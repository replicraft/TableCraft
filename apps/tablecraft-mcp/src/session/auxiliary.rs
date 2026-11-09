//! Actual Find descriptors remain in the host; JSON cannot invent a Replace target.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use tablecraft_display::{CurrencyChoice, DateFormat, Options, TimeFormat};
use tablecraft_runtime::{
    Command, FilterHandle, FindDirection, FindMatch, FindPosition, FindRequest, FindScope,
    FindTarget, Response,
};

const MAX_LOCALE_BYTES: usize = 128;

#[derive(Default)]
pub(super) struct FindState {
    found: Option<Found>,
}

struct Found {
    id: u64,
    request: FindRequest,
    position: FindPosition,
    filter: Option<(FilterHandle, bool)>,
}

impl Session {
    pub(super) fn find_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let revision = self.current_revision(args)?;
        let (command, filter) = match operation {
            "find_next" => {
                let formatted = arg::boolean(args, "formatted", false)?;
                let format_options = match arg::optional(args, "format_options") {
                    Some(options) => format_options(options)?,
                    None if formatted => {
                        return Err("Formatted Find requires explicit format_options".into());
                    }
                    None => Options::default(),
                };
                let table = arg::owned(args, "table")?;
                let filter = self.view_handle(args, revision, Some(&table))?;
                let enabled = arg::boolean(args, "enabled", filter.is_some())?;
                let filter = filter.map(|handle| (handle, enabled));
                let scope = match arg::optional(args, "column") {
                    None => FindScope::Table,
                    Some(_) => FindScope::Field(arg::owned(args, "column")?),
                };
                let matching = match arg::text(args, "matching")? {
                    "any" => FindMatch::Any,
                    "whole" => FindMatch::Whole,
                    "start" => FindMatch::Start,
                    _ => return Err("matching must be any, whole or start".into()),
                };
                let direction = match arg::text(args, "direction")? {
                    "all" => FindDirection::All,
                    "up" => FindDirection::Up,
                    "down" => FindDirection::Down,
                    _ => return Err("direction must be all, up or down".into()),
                };
                let request = FindRequest {
                    target: FindTarget {
                        document_revision: revision,
                        table,
                    },
                    scope,
                    pattern: arg::owned(args, "pattern")?,
                    matching,
                    direction,
                    start: None,
                    match_case: arg::boolean(args, "match_case", false)?,
                    formatted,
                    format_options,
                };
                self.find_state = FindState::default();
                (find_command(request, &filter), filter)
            }
            "continue_find" => {
                let mut found = self.consume_found(args, revision)?;
                found.request.start = Some(found.position);
                (find_command(found.request, &found.filter), found.filter)
            }
            "replace_found" => {
                // Validate input before consuming the only actual occurrence descriptor.
                let replacement = arg::owned(args, "replacement")?;
                let found = self.consume_found(args, revision)?;
                (
                    Command::ReplaceFindMatch {
                        request: found.request,
                        found: found.position,
                        replacement,
                        filter: found.filter,
                    },
                    None,
                )
            }
            _ => return Err("Unknown Find operation".into()),
        };
        let response = self.request(command)?;
        match response {
            Response::Found { request, position } => {
                if request.target.document_revision != revision {
                    return Err("Find returned a different document revision".into());
                }
                let Some(position) = position else {
                    self.find_state = FindState::default();
                    return Ok(json!({"document_revision": revision, "found": false}));
                };
                let id = self.next_handle()?;
                let result = json!({
                    "document_revision": revision,
                    "find_id": id,
                    "found": true,
                    "formatted": request.formatted,
                    "filtered": filter.is_some(),
                    "filter_enabled": filter.as_ref().map(|(_, enabled)| *enabled),
                    "table": request.target.table,
                    "parent_ordinal": position.parent_ordinal,
                    "column": position.column,
                    "span": position.span.as_ref().map(|span| json!({
                        "start_utf16": span.start_utf16,
                        "end_utf16": span.end_utf16,
                    })),
                });
                self.find_state.found = Some(Found {
                    id,
                    request,
                    position,
                    filter,
                });
                Ok(result)
            }
            Response::EditDraft(draft) if operation == "replace_found" => {
                self.response(Response::EditDraft(draft))
            }
            _ => Err("The runtime returned a response outside the Find operation contract".into()),
        }
    }

    fn consume_found(&mut self, args: &Value, revision: u64) -> Result<Found, String> {
        let id = arg::u64_value(args, "find_id")?;
        if !self.find_state.found.as_ref().is_some_and(|found| {
            found.id == id && found.request.target.document_revision == revision
        }) {
            return Err("Unknown, consumed, superseded or stale Find handle".into());
        }
        self.find_state
            .found
            .take()
            .ok_or("Find handle is no longer owned".into())
    }
}

fn find_command(request: FindRequest, filter: &Option<(FilterHandle, bool)>) -> Command {
    match filter {
        Some((handle, enabled)) => {
            // NO_CLONE: The descriptor retains the original immutable filtered row map for
            // continuation/replacement while the worker searches the same shared map.
            // FilterHandle is Arc-backed; this creates no independent row-map payload.
            let handle = handle.clone();
            Command::FindFiltered {
                handle,
                request,
                enabled: *enabled,
            }
        }
        None => Command::FindNext { request },
    }
}

fn format_options(args: &Value) -> Result<Options, String> {
    let locale = arg::text(args, "system_locale")?;
    if locale.is_empty() || locale.len() > MAX_LOCALE_BYTES || locale.chars().any(char::is_control)
    {
        return Err("system_locale must be a nonempty locale tag of at most 128 bytes".into());
    }
    let date = match arg::text(args, "date")? {
        "system_locale" => DateFormat::SystemLocale,
        "iso" => DateFormat::Iso,
        "day_month_year" => DateFormat::DayMonthYear,
        "month_day_year" => DateFormat::MonthDayYear,
        "short" => DateFormat::Short,
        "long" => DateFormat::Long,
        "raw" => DateFormat::Raw,
        _ => return Err("Unsupported date display choice".into()),
    };
    let time = match arg::text(args, "time")? {
        "system_locale" => TimeFormat::SystemLocale,
        "hours_minutes" => TimeFormat::HoursMinutes,
        "hours_minutes_seconds" => TimeFormat::HoursMinutesSeconds,
        "twelve_hour" => TimeFormat::TwelveHour,
        "fractional" => TimeFormat::Fractional,
        "raw" => TimeFormat::Raw,
        _ => return Err("Unsupported time display choice".into()),
    };
    let currency = match arg::text(args, "currency")? {
        "system_locale" => CurrencyChoice::SystemLocale,
        "raw" => CurrencyChoice::Raw,
        "usd" => CurrencyChoice::Usd,
        "eur" => CurrencyChoice::Eur,
        "gbp" => CurrencyChoice::Gbp,
        "jpy" => CurrencyChoice::Jpy,
        "krw" => CurrencyChoice::Krw,
        "cny" => CurrencyChoice::Cny,
        "brl" => CurrencyChoice::Brl,
        "rub" => CurrencyChoice::Rub,
        "cad" => CurrencyChoice::Cad,
        "aud" => CurrencyChoice::Aud,
        "inr" => CurrencyChoice::Inr,
        "chf" => CurrencyChoice::Chf,
        "kwd" => CurrencyChoice::Kwd,
        "bhd" => CurrencyChoice::Bhd,
        "tnd" => CurrencyChoice::Tnd,
        _ => return Err("Unsupported currency display choice".into()),
    };
    Ok(Options {
        system_locale: Some(arg::owned(args, "system_locale")?),
        date,
        time,
        currency,
    })
}
