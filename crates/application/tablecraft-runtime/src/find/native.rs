//! Complete local-table cursor traversal; no SQL filters or loaded-page searches.
use super::{
    FindDirection, FindError, FindPosition, FindRequest, FindScope, FindSpan, FindTarget,
    MAX_FIND_PATTERN_BYTES, matching::Matcher,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    rows::presentation::Metadata,
};
use std::{cell::RefCell, cmp::Ordering};
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{
    DatabaseQueryMode, inspect_database_properties, inspect_database_sort_order,
    inspect_table_schema,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_display::Options;
use tablecraft_expressions::{
    comparison::StringSearch,
    predicates::{LikeMode, next_like_offset, previous_like_offset},
};
use tablecraft_schema::ScalarType;

const SCAN_BATCH_ROWS: u32 = 100;
const MATCH_BYTES: u64 = 64 * 1024 * 1024;
const MATCH_STEPS: u64 = 10_000_000;
const HIGH_SURROGATES: std::ops::RangeInclusive<u16> = 0xd800..=0xdbff;
const LOW_SURROGATES: std::ops::RangeInclusive<u16> = 0xdc00..=0xdfff;

fn boundary(units: &[u16], offset: usize) -> bool {
    offset <= units.len()
        && !(offset > 0
            && offset < units.len()
            && HIGH_SURROGATES.contains(&units[offset - 1])
            && LOW_SURROGATES.contains(&units[offset]))
}

fn copy_position(
    position: &FindPosition,
    output: &mut Presentation<'_>,
) -> Result<FindPosition, Error> {
    Ok(FindPosition {
        parent_ordinal: position.parent_ordinal,
        column: output.text(&position.column)?,
        span: position.span.clone(),
    })
}

fn copy_request(
    request: &FindRequest,
    output: &mut Presentation<'_>,
) -> Result<FindRequest, Error> {
    Ok(FindRequest {
        target: FindTarget {
            document_revision: request.target.document_revision,
            table: output.text(&request.target.table)?,
        },
        scope: match &request.scope {
            FindScope::Field(name) => FindScope::Field(output.text(name)?),
            FindScope::Table => FindScope::Table,
        },
        pattern: output.text(&request.pattern)?,
        matching: request.matching,
        direction: request.direction,
        start: request
            .start
            .as_ref()
            .map(|position| copy_position(position, output))
            .transpose()?,
        match_case: request.match_case,
        formatted: request.formatted,
        format_options: Options {
            system_locale: request
                .format_options
                .system_locale
                .as_deref()
                .map(|locale| output.text(locale))
                .transpose()?,
            date: request.format_options.date,
            time: request.format_options.time,
            currency: request.format_options.currency,
        },
    })
}

impl Document {
    pub(crate) fn find_next(
        &mut self,
        request: &FindRequest,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if request.target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if request.pattern.is_empty() {
            return Err(FindError::EmptyPattern.into());
        }
        if request.pattern.len() > MAX_FIND_PATTERN_BYTES {
            return Err(FindError::PatternLimit.into());
        }
        let mut output = Presentation::new(cancelled);
        let captured_request = copy_request(request, &mut output)?;
        let request = &captured_request;
        let mut native_work = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &request.target.table,
            &mut native_work,
            cancelled,
        )?;
        let anchor_column = request
            .start
            .as_ref()
            .map(|anchor| {
                schema
                    .columns()
                    .iter()
                    .position(|field| field.name() == anchor.column)
                    .ok_or(FindError::InvalidTarget)
            })
            .transpose()?;
        if request.start.as_ref().is_some_and(|anchor| {
            anchor
                .span
                .as_ref()
                .is_some_and(|span| span.start_utf16 > span.end_utf16)
        }) {
            return Err(FindError::InvalidTarget.into());
        }
        let metadata = Metadata::inspect(
            &mut self.source,
            &request.target.table,
            &mut native_work,
            cancelled,
        )?;
        let mode = match inspect_database_properties(
            &mut self.source,
            SCOPE,
            &mut native_work,
            cancelled,
        )? {
            Some(properties) => properties.query_mode(&mut native_work, cancelled)?,
            None => DatabaseQueryMode::Absent,
        };
        let mode = match mode {
            DatabaseQueryMode::Ansi89 | DatabaseQueryMode::Absent => LikeMode::Ansi89,
            DatabaseQueryMode::Ansi92 => LikeMode::Ansi92,
            DatabaseQueryMode::Unknown => return Err(FindError::WildcardMode.into()),
        };
        let order = inspect_database_sort_order(&mut self.source, &mut native_work, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let mut work = ResourceBudget::new(MATCH_BYTES, 0, MATCH_STEPS);
        let matcher = Matcher::new(request, mode, &mut work, cancelled)?;
        let mut positions = Vec::new();
        output.allocation(schema.columns().len(), size_of::<usize>())?;
        positions.try_reserve_exact(schema.columns().len())?;
        let mut selected_field_found = matches!(request.scope, FindScope::Table);
        for (position, field) in schema.columns().iter().enumerate() {
            check(cancelled)?;
            if let FindScope::Field(name) = &request.scope {
                if field.name() != name {
                    continue;
                }
                selected_field_found = true;
            }
            // These are documented Find exclusions, not successful searches of
            // uninspected data. Unsupported declarations otherwise refuse.
            if field.is_ole_object() || field.scalar_type() == Some(ScalarType::Boolean) {
                continue;
            }
            let scalar = field.scalar_type().ok_or(FindError::UnsupportedField)?;
            if scalar == ScalarType::Binary {
                return Err(FindError::UnsupportedField.into());
            }
            metadata.require_find_format(
                position,
                field.name(),
                scalar,
                request.formatted,
                &mut output,
            )?;
            matcher.require_type(scalar)?;
            if let Some(order) = field.text_sort_order() {
                resolver.resolve_native(order)?;
            }
            positions.push(position);
        }
        if !selected_field_found {
            return Err(FindError::InvalidTarget.into());
        }
        let mut columns = output.columns(positions.len())?;
        let mut names = Vec::new();
        output.allocation(positions.len(), size_of::<&str>())?;
        names.try_reserve_exact(positions.len())?;
        for &position in &positions {
            let name = schema.columns()[position].name();
            names.push(name);
            columns.push(metadata.column(position, name, &mut output)?);
        }
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native_work, cancelled);
        let mut cursor = browser.projected_cursor(&request.target.table, &names)?;
        let mut primary = None;
        let mut wrapped = None;
        let mut anchor_seen = request.start.is_none();
        loop {
            let batch = cursor.next_page(SCAN_BATCH_ROWS)?;
            for (row_number, values) in batch.rows().iter().enumerate() {
                check(cancelled)?;
                let ordinal = batch
                    .offset()
                    .checked_add(u32::try_from(row_number).map_err(|_| FindError::InvalidTarget)?)
                    .ok_or(FindError::InvalidTarget)?;
                if request
                    .start
                    .as_ref()
                    .is_some_and(|anchor| anchor.parent_ordinal == ordinal)
                {
                    anchor_seen = true;
                }
                let mut cells = output.row(values.len())?;
                for value in values {
                    cells.push(output.scalar(value, false)?);
                }
                let mut rows = output.rows(1)?;
                rows.push(cells);
                metadata.enrich_projection(&positions, &columns, &mut rows, &mut output)?;
                let mut cells = rows.pop().ok_or(FindError::InvalidTarget)?;
                for (projected, (&position, (value, cell))) in positions
                    .iter()
                    .zip(values.iter().zip(cells.iter_mut()))
                    .enumerate()
                {
                    check(cancelled)?;
                    let relation = match (&request.start, anchor_column) {
                        (Some(anchor), Some(column)) => {
                            (ordinal, position).cmp(&(anchor.parent_ordinal, column))
                        }
                        _ => {
                            if request.direction == FindDirection::Up {
                                Ordering::Less
                            } else {
                                Ordering::Greater
                            }
                        }
                    };
                    let policy = match schema.columns()[position].text_sort_order() {
                        Some(order) => resolver.resolve_native(order)?,
                        None => resolver.default_collation(),
                    };
                    let special = matcher.special(value);
                    let text = if special.is_some() {
                        ""
                    } else {
                        matcher.text(value, cell, &mut output)?
                    };
                    let continuation = (relation == Ordering::Equal)
                        .then(|| {
                            request
                                .start
                                .as_ref()
                                .and_then(|anchor| anchor.span.as_ref())
                        })
                        .flatten();
                    let units = matcher.units(text, &mut work, cancelled)?;
                    if continuation.is_some_and(|span| {
                        !boundary(&units, span.start_utf16 as usize)
                            || !boundary(&units, span.end_utf16 as usize)
                    }) {
                        return Err(FindError::InvalidTarget.into());
                    }
                    let forward_start = match continuation {
                        Some(span) if span.start_utf16 == span.end_utf16 => {
                            next_like_offset(&units, span.end_utf16 as usize)
                        }
                        Some(span) => Some(span.end_utf16 as usize),
                        None => Some(0),
                    };
                    let reverse_end = match continuation {
                        Some(span) if span.start_utf16 == span.end_utf16 => {
                            previous_like_offset(&units, span.start_utf16 as usize)
                        }
                        Some(span) => Some(span.start_utf16 as usize),
                        None => Some(units.len()),
                    };
                    let mut find = |search| -> Result<Option<FindSpan>, Error> {
                        match special {
                            Some(true) => Ok(Some(FindSpan {
                                start_utf16: 0,
                                end_utf16: 0,
                            })),
                            Some(false) => Ok(None),
                            None => matcher.find(text, search, policy, &mut work, cancelled),
                        }
                    };
                    let hit = match request.direction {
                        FindDirection::Up if relation != Ordering::Greater => {
                            if special.is_some() && continuation.is_some() {
                                None
                            } else {
                                reverse_end
                                    .map(|end| find(StringSearch::ReverseThrough(end)))
                                    .transpose()?
                                    .flatten()
                            }
                        }
                        FindDirection::Down | FindDirection::All
                            if relation != Ordering::Less && primary.is_none() =>
                        {
                            if special.is_some() && continuation.is_some() {
                                None
                            } else {
                                forward_start
                                    .map(|start| find(StringSearch::ForwardFrom(start)))
                                    .transpose()?
                                    .flatten()
                            }
                        }
                        _ => None,
                    };
                    if let Some(span) = hit {
                        primary = Some(FindPosition {
                            parent_ordinal: ordinal,
                            column: output.text(&columns[projected].name)?,
                            span: Some(span),
                        });
                    }
                    if request.direction == FindDirection::All
                        && relation != Ordering::Greater
                        && wrapped.is_none()
                        && let Some(span) = find(StringSearch::ForwardFrom(0))?
                    {
                        wrapped = Some(FindPosition {
                            parent_ordinal: ordinal,
                            column: output.text(&columns[projected].name)?,
                            span: Some(span),
                        });
                    }
                }
            }
            if batch.reached_end() {
                break;
            }
        }
        if !anchor_seen {
            return Err(FindError::InvalidTarget.into());
        }
        Ok(Response::Found {
            request: captured_request,
            position: primary.or(wrapped),
        })
    }
}
