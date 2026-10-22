//! Bounded UTF-16 substring scans and explicit locale/provider delegation.
mod mode;
use super::{TextFunction, TextResult, compare};
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
    functions::conversions::Conversion,
};
use mode::SearchMode;
use tablecraft_core::values::Value;

const FIRST_POSITION: i32 = 1;
const LAST_POSITION: i32 = -1;

pub(super) fn apply(
    function: TextFunction,
    arguments: &[Value],
    collation: Option<&dyn Collation>,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<TextResult<'static>, EvalError> {
    let search = prepare(function, arguments)?;
    let start = match search.start {
        Some(value) => match Conversion::Long.apply(value)? {
            Value::Int32(value) => value,
            _ => return Err(EvalError::TypeMismatch),
        },
        None => search.default_start(),
    };
    search.find(start, collation, step)
}

pub(crate) struct Search<'a> {
    start: Option<&'a Value>,
    haystack: &'a Value,
    needle: &'a Value,
    mode: SearchMode,
    reverse: bool,
}

#[cfg(feature = "sql")]
pub(crate) fn start_operand(function: TextFunction, arguments: &[Value]) -> Option<&Value> {
    if function == TextFunction::InStrRev {
        arguments.get(2)
    } else if arguments.len() > 2 {
        arguments.first()
    } else {
        None
    }
}

pub(crate) fn prepare(
    function: TextFunction,
    arguments: &[Value],
) -> Result<Search<'_>, EvalError> {
    prepare_mode(function, arguments, None)
}

pub(crate) fn prepare_mode<'a>(
    function: TextFunction,
    arguments: &'a [Value],
    normalized_mode: Option<&Value>,
) -> Result<Search<'a>, EvalError> {
    let reverse = function == TextFunction::InStrRev;
    let (start, haystack, needle, mode) = if reverse {
        (
            arguments.get(2),
            &arguments[0],
            &arguments[1],
            SearchMode::Compare(compare::mode(
                normalized_mode.or_else(|| arguments.get(3)),
                Some(StringCompareMode::Binary),
            )?),
        )
    } else if arguments.len() == 2 {
        (
            None,
            &arguments[0],
            &arguments[1],
            SearchMode::Compare(None),
        )
    } else {
        (
            arguments.first(),
            &arguments[1],
            &arguments[2],
            mode::forward(normalized_mode.or_else(|| arguments.get(3)))?,
        )
    };
    Ok(Search {
        start,
        haystack,
        needle,
        mode,
        reverse,
    })
}

impl Search<'_> {
    #[cfg(feature = "sql")]
    pub(crate) fn start(&self) -> Option<&Value> {
        self.start
    }

    fn default_start(&self) -> i32 {
        if self.reverse {
            LAST_POSITION
        } else {
            FIRST_POSITION
        }
    }

    pub(crate) fn find(
        self,
        start: i32,
        collation: Option<&dyn Collation>,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<TextResult<'static>, EvalError> {
        let Self {
            haystack,
            needle,
            mode,
            reverse,
            ..
        } = self;
        if start < FIRST_POSITION && !(reverse && start == LAST_POSITION) {
            return Err(EvalError::Domain);
        }
        if matches!(haystack, Value::Null) || matches!(needle, Value::Null) {
            return Ok(TextResult::Null);
        }
        let (Value::Text(haystack), Value::Text(needle)) = (haystack, needle) else {
            return Err(EvalError::TypeMismatch);
        };
        let length = units(haystack, step)?;
        let needle_length = units(needle, step)?;
        let start = if reverse && start == LAST_POSITION {
            length
        } else {
            start as usize
        };
        if length == 0 || start > length {
            return Ok(TextResult::Position(0));
        }
        if needle_length == 0 {
            return result(Some(start - 1));
        }
        let search = if reverse {
            StringSearch::ReverseThrough(start)
        } else {
            StringSearch::ForwardFrom(start - 1)
        };
        let mode = mode.resolve(collation)?;
        let found = if matches!(mode, SearchMode::Compare(Some(StringCompareMode::Binary))) {
            binary(haystack, needle, length, needle_length, search, step)?
        } else {
            step()?;
            let collation = collation.ok_or(EvalError::CollationRequired)?;
            let found = match mode {
                SearchMode::Locale(locale) => {
                    collation.find_locale(haystack, needle, search, locale)?
                }
                SearchMode::Compare(Some(mode)) => {
                    collation.find(haystack, needle, search, mode)?
                }
                SearchMode::Compare(None) => return Err(EvalError::CollationRequired),
            };
            step()?;
            if let Some(found) = &found {
                let valid = found.start <= found.end
                    && found.start < length
                    && found.end <= length
                    && match search {
                        StringSearch::ForwardFrom(start) => found.start >= start,
                        StringSearch::ReverseThrough(end) => found.end <= end,
                    };
                if !valid {
                    return Err(EvalError::InvalidTextMatch);
                }
            }
            found.map(|found| found.start)
        };
        result(found)
    }
}
fn units(text: &str, step: &mut impl FnMut() -> Result<(), EvalError>) -> Result<usize, EvalError> {
    let mut count = 0usize;
    for _ in text.encode_utf16() {
        step()?;
        count = count.checked_add(1).ok_or(EvalError::Overflow)?;
    }
    i32::try_from(count).map_err(|_| EvalError::Overflow)?;
    Ok(count)
}
fn result(position: Option<usize>) -> Result<TextResult<'static>, EvalError> {
    let position = match position {
        Some(position) => position.checked_add(1).ok_or(EvalError::Overflow)?,
        None => 0,
    };
    Ok(TextResult::Position(
        i32::try_from(position).map_err(|_| EvalError::Overflow)?,
    ))
}
fn binary(
    haystack: &str,
    needle: &str,
    length: usize,
    needle_length: usize,
    search: StringSearch,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<Option<usize>, EvalError> {
    let (first, end, reverse) = match search {
        StringSearch::ForwardFrom(start) => (start, length, false),
        StringSearch::ReverseThrough(end) => (0, end, true),
    };
    let Some(last) = end.checked_sub(needle_length) else {
        return Ok(None);
    };
    let mut candidates = haystack.encode_utf16();
    let mut found = None;
    for position in 0..=last {
        step()?;
        let mut candidate = candidates.clone();
        candidates.next();
        if position < first {
            continue;
        }
        let mut matched = true;
        for unit in needle.encode_utf16() {
            step()?;
            if candidate.next() != Some(unit) {
                matched = false;
                break;
            }
        }
        if matched {
            found = Some(position);
            if !reverse {
                break;
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod locale_tests;
