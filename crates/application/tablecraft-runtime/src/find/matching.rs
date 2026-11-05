//! One compiled pattern and captured shared display policy for a native scan.
use super::{FindError, FindMatch, FindRequest, FindSpan};
use crate::{Cell, Error, controls::SCOPE, display::Presentation};
use std::ops::Range;
use tablecraft_ace::{DatabaseQueryMode, NativeReadSource, inspect_database_properties};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_display::{Engine, NativeFormat, ScalarPresentation};
use tablecraft_expressions::{
    comparison::{Collation, StringSearch},
    predicates::{LikeCase, LikeMatch, LikeMode, LikeString, PreparedLikeSearch},
};
use tablecraft_schema::ScalarType;

pub(crate) fn query_mode<R: NativeReadSource>(
    source: &mut R,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeMode, Error> {
    let mode = match inspect_database_properties(source, SCOPE, work, cancelled)? {
        Some(properties) => properties.query_mode(work, cancelled)?,
        None => DatabaseQueryMode::Absent,
    };
    match mode {
        DatabaseQueryMode::Absent | DatabaseQueryMode::Ansi89 => Ok(LikeMode::Ansi89),
        DatabaseQueryMode::Ansi92 => Ok(LikeMode::Ansi92),
        DatabaseQueryMode::Unknown => Err(FindError::WildcardMode.into()),
    }
}

pub(crate) struct Matcher {
    pattern: PreparedLikeSearch,
    null: bool,
    empty: bool,
    formatted: bool,
    engine: Option<Engine>,
}

impl Matcher {
    pub(crate) fn new(
        request: &FindRequest,
        mode: LikeMode,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let matching = match request.matching {
            FindMatch::Any => LikeMatch::Any,
            FindMatch::Whole => LikeMatch::Whole,
            FindMatch::Start => LikeMatch::Start,
        };
        let case = if request.match_case {
            LikeCase::Sensitive
        } else {
            LikeCase::Database
        };
        let typed = !request.formatted && request.matching == FindMatch::Whole;
        let null = typed
            && (request.pattern.eq_ignore_ascii_case("Null")
                || request.pattern.eq_ignore_ascii_case("Is Null"));
        let empty = typed && request.pattern == "\"\"";
        let pattern = PreparedLikeSearch::new(
            LikeString::Text(&request.pattern),
            mode,
            matching,
            case,
            work,
            Some(cancelled),
        )?;
        let engine = request
            .formatted
            .then(|| Engine::new(&request.format_options))
            .transpose()
            .map_err(|_| FindError::UnsupportedFormat)?;
        Ok(Self {
            pattern,
            null,
            empty,
            formatted: request.formatted,
            engine,
        })
    }

    pub(crate) fn require_type(&self, scalar: ScalarType) -> Result<(), Error> {
        if self.formatted || self.null || self.empty {
            return Ok(());
        }
        match scalar {
            ScalarType::DateTime | ScalarType::DateTimeExtended => {
                Err(FindError::UnformattedDate.into())
            }
            ScalarType::Currency
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::Decimal => Err(FindError::UnformattedNumber.into()),
            _ => Ok(()),
        }
    }

    pub(crate) fn units(
        &self,
        text: &str,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u16>, Error> {
        work.charge_allocation(text.len() as u64, size_of::<u16>() as u64)?;
        let mut units = Vec::new();
        units.try_reserve_exact(text.len())?;
        for unit in text.encode_utf16() {
            crate::controls::check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            units.push(unit);
        }
        Ok(units)
    }

    pub(crate) fn special(&self, value: &Value) -> Option<bool> {
        if self.null {
            return Some(matches!(value, Value::Null));
        }
        if self.empty {
            return Some(matches!(value, Value::Text(text) if text.is_empty()));
        }
        if matches!(value, Value::Null) {
            return Some(false);
        }
        None
    }

    pub(crate) fn text<'a>(
        &self,
        value: &'a Value,
        cell: &'a mut Cell,
        output: &mut Presentation<'_>,
    ) -> Result<&'a str, Error> {
        if self.formatted {
            if let Some(presentation) = cell.presentation {
                if matches!(
                    presentation,
                    ScalarPresentation::Currency {
                        native_format: NativeFormat::Unsupported,
                        ..
                    } | ScalarPresentation::DateTime {
                        native_format: NativeFormat::Unsupported,
                        ..
                    }
                ) {
                    return Err(FindError::UnsupportedFormat.into());
                }
                let engine = self.engine.as_ref().ok_or(FindError::UnsupportedFormat)?;
                cell.text = output.text(&engine.format(presentation))?;
            }
            return Ok(&cell.text);
        }
        // Unformatted dates use regional parsing in Access; display/raw carrier
        // strings cannot stand in for that contract.
        if matches!(value, Value::DateTime(_) | Value::DateTimeExtended(_)) {
            return Err(FindError::UnformattedDate.into());
        }
        Ok(match value {
            Value::Text(text) => text,
            _ => &cell.text,
        })
    }

    pub(crate) fn find(
        &self,
        text: &str,
        bounds: StringSearch,
        collation: &dyn Collation,
        work: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<FindSpan>, Error> {
        self.pattern
            .find(
                LikeString::Text(text),
                bounds,
                collation,
                work,
                Some(cancelled),
            )?
            .map(span)
            .transpose()
    }
}

fn span(range: Range<usize>) -> Result<FindSpan, Error> {
    Ok(FindSpan {
        start_utf16: u32::try_from(range.start).map_err(|_| FindError::InvalidTarget)?,
        end_utf16: u32::try_from(range.end).map_err(|_| FindError::InvalidTarget)?,
    })
}
