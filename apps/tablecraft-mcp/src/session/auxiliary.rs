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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use tablecraft_testkit::fixtures::access::mutation_fixture;

    fn source() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>, Session) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("find.accdb");
        let before =
            mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]);
        std::fs::write(&path, &before).unwrap();
        let mut session = Session::new().unwrap();
        session
            .execute("open_database", json!({"path": path}))
            .unwrap();
        (directory, path, before, session)
    }

    fn find(session: &mut Session, pattern: &str) -> Value {
        session
            .execute(
                "find_next",
                json!({
                    "document_revision": session.revision.unwrap(), "table": "Items",
                    "column": "Score", "pattern": pattern, "matching": "any", "direction": "all",
                }),
            )
            .unwrap()
    }

    #[test]
    fn real_occurrence_becomes_normal_edit_review_and_native_save_without_invented_target() {
        let (_directory, path, before, mut session) = source();
        let revision = session.revision.unwrap();
        let found = find(&mut session, "2");
        assert_eq!(found["parent_ordinal"], 1);
        assert_eq!(found["column"], "Score");
        assert_eq!(found["span"]["start_utf16"], 0);
        let draft = session
            .execute(
                "replace_found",
                json!({
                    "document_revision": revision, "find_id": found["find_id"], "replacement": "7",
                }),
            )
            .unwrap();
        assert_eq!(draft["parent_ordinal"], 1);
        assert_eq!(draft["initial"]["text"], "70");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(session.execute("replace_found", json!({
            "document_revision": revision, "find_id": found["find_id"], "replacement": "9",
        })).is_err());
        let reviewed = session
            .execute(
                "review_edit",
                json!({
                    "document_revision": revision, "draft_id": draft["draft_id"], "text": "70",
                }),
            )
            .unwrap();
        let started = session
            .execute("apply_edit", json!({"review_id": reviewed["review_id"]}))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let status = session
                .execute("save_status", json!({"save_id": started["save_id"]}))
                .unwrap();
            if status["status"] == "terminal" {
                assert_eq!(status["outcome"]["kind"], "saved", "{status}");
                break;
            }
            assert!(Instant::now() < deadline, "Find replacement save stalled");
            std::thread::yield_now();
        }
        let acknowledgement = session
            .execute("acknowledge_save", json!({"save_id": started["save_id"]}))
            .unwrap();
        assert_eq!(acknowledgement["reopened"], true);
        let page = session
            .execute(
                "read_rows",
                json!({
                    "document_revision": session.revision.unwrap(), "table": "Items", "offset": 0,
                }),
            )
            .unwrap();
        assert_eq!(page["rows"][1][1]["text"], "70");
        assert_ne!(session.revision, Some(revision));
    }

    #[test]
    fn continuation_consumes_actual_handle_and_unknown_stale_or_formatted_requests_do_not_write() {
        let (_directory, path, before, mut session) = source();
        let revision = session.revision.unwrap();
        let first = find(&mut session, "0");
        assert_eq!(first["parent_ordinal"], 0);
        assert!(
            session
                .execute(
                    "replace_found",
                    json!({
                        "document_revision": revision, "find_id": 999, "replacement": "5",
                    })
                )
                .is_err()
        );
        assert!(
            session
                .execute(
                    "continue_find",
                    json!({
                        "document_revision": revision + 1, "find_id": first["find_id"],
                    })
                )
                .is_err()
        );
        let second = session
            .execute(
                "continue_find",
                json!({
                    "document_revision": revision, "find_id": first["find_id"],
                }),
            )
            .unwrap();
        assert_eq!(second["parent_ordinal"], 1);
        assert_ne!(second["find_id"], first["find_id"]);
        assert!(
            session
                .execute(
                    "continue_find",
                    json!({
                        "document_revision": revision, "find_id": first["find_id"],
                    })
                )
                .is_err()
        );
        assert!(
            session
                .execute(
                    "find_next",
                    json!({
                        "document_revision": revision, "table": "Items", "pattern": "0",
                        "matching": "whole", "direction": "all", "formatted": true,
                    })
                )
                .is_err()
        );
        assert!(!session.saves.is_busy());
        assert!(session.active_save.is_none());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn filtered_sorted_occurrence_retains_its_real_view_through_continuation_and_replacement() {
        use crate::session::tests::{acknowledge, opened, save};
        let before =
            mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]);
        let (_directory, path, mut session) = opened(&before);
        let revision = session.revision.unwrap();
        let filtered = session
            .execute(
                "apply_field_filter",
                json!({
                    "document_revision": revision, "table": "Items", "column": "Score",
                    "operator": "greater_or_equal", "inputs": ["20"],
                }),
            )
            .unwrap();
        let sorted = session.execute("sort_view", json!({
            "document_revision": revision, "table": "Items", "filter_id": filtered["filter_id"],
            "sort": {"column": "Score", "descending": true},
        })).unwrap();
        let first = session.execute("find_next", json!({
            "document_revision": revision, "table": "Items", "column": "Score", "pattern": "0",
            "matching": "any", "direction": "all", "filter_id": sorted["filter_id"],
        })).unwrap();
        assert_eq!(first["parent_ordinal"], 2);
        assert!(session.find_state.found.as_ref().unwrap().filter.is_some());

        // Publishing another page supersedes the protocol filter ID, while the Find
        // descriptor still owns the original real sorted view, never a JSON row map.
        session
            .execute(
                "filter_page",
                json!({
                    "document_revision": revision, "filter_id": sorted["filter_id"], "offset": 0,
                }),
            )
            .unwrap();
        let next = session
            .execute(
                "continue_find",
                json!({
                    "document_revision": revision, "find_id": first["find_id"],
                }),
            )
            .unwrap();
        assert_eq!(next["parent_ordinal"], 1);
        let draft = session
            .execute(
                "replace_found",
                json!({
                    "document_revision": revision, "find_id": next["find_id"], "replacement": "7",
                }),
            )
            .unwrap();
        assert_eq!(draft["parent_ordinal"], 1);
        assert_eq!(draft["initial"]["text"], "27");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let reviewed = session
            .execute(
                "review_edit",
                json!({
                    "document_revision": revision, "draft_id": draft["draft_id"], "text": "27",
                }),
            )
            .unwrap();
        let id = save(&mut session, "apply_edit", reviewed);
        acknowledge(&mut session, id, &path);
        let rows = session
            .execute(
                "read_rows",
                json!({
                    "document_revision": session.revision.unwrap(), "table": "Items", "offset": 0,
                }),
            )
            .unwrap();
        assert_eq!(rows["rows"][0][1]["text"], "10");
        assert_eq!(rows["rows"][1][1]["text"], "27");
        assert_eq!(rows["rows"][2][1]["text"], "30");
    }

    #[test]
    fn formatted_currency_and_date_find_use_only_the_explicit_captured_display_policy() {
        use crate::session::tests::opened;
        use tablecraft_testkit::fixtures::access::{
            canonical_fixed_creation, canonical_native17_creation,
        };
        let source = canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
        let before = canonical_native17_creation::source(source.bytes);
        let (_directory, path, mut session) = opened(&before);
        let revision = session.revision.unwrap();
        let mut request = json!({
            "document_revision": revision, "table": "Items", "column": "Money", "pattern": "€0.00",
            "matching": "whole", "direction": "all", "formatted": true,
            "format_options": {"system_locale": "en-US", "date": "iso", "time": "hours_minutes_seconds", "currency": "eur"},
        });
        let money = session.execute("find_next", request.clone()).unwrap();
        assert_eq!(money["found"], true);
        assert_eq!(money["column"], "Money");
        let captured = &session.find_state.found.as_ref().unwrap().request;
        assert!(captured.formatted);
        assert_eq!(captured.format_options.currency, CurrencyChoice::Eur);
        request["format_options"]["currency"] = json!("usd");
        assert_eq!(
            session.execute("find_next", request.clone()).unwrap()["found"],
            false
        );
        request["column"] = json!("DateValue");
        request["pattern"] = json!("1899-12-30");
        request["matching"] = json!("any");
        assert_eq!(
            session.execute("find_next", request.clone()).unwrap()["found"],
            true
        );
        request["format_options"]["date"] = json!("month_day_year");
        assert_eq!(
            session.execute("find_next", request.clone()).unwrap()["found"],
            false
        );
        request["pattern"] = json!("12/30/1899");
        assert_eq!(
            session.execute("find_next", request.clone()).unwrap()["found"],
            true
        );
        request["format_options"]["time"] = json!("invented");
        assert!(session.execute("find_next", request).is_err());
        assert!(!session.saves.is_busy());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
