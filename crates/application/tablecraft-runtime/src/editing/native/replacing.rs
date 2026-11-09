use crate::editing::replacing::ReplaceError;
use crate::editing::{EditError, EditInput, EditTarget, platform::input};
use crate::{
    Cell, Error, FilterHandle, FindDirection, FindError, FindPosition, FindRequest, FindScope,
    FindSpan, MAX_FIND_PATTERN_BYTES, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    find::matching::{self, Matcher},
    rows::presentation::Metadata,
    scalar_input::MAX_INPUT_BYTES,
};
use std::{cell::RefCell, ops::Range};
use tablecraft_access::{NativeCollationResolver, editing::journal::NativeJournalEditor};
use tablecraft_ace::{NativeImage, inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{EditableTable, UpdateProfile};
use tablecraft_expressions::comparison::StringSearch;
use tablecraft_schema::{ScalarType, TypeDescriptor};

const MATCH_BYTES: u64 = 64 * 1024 * 1024;
const MATCH_STEPS: u64 = 10_000_000;

impl Document {
    pub(crate) fn replace_find_match(
        &mut self,
        request: &FindRequest,
        found: &FindPosition,
        replacement: &str,
        filter: Option<(&FilterHandle, bool)>,
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
        if replacement.len() > MAX_INPUT_BYTES {
            return Err(EditError::InputLimit.into());
        }
        if !self.source.editable_native() {
            return Err(EditError::ReadOnlySource.into());
        }
        if let Some((handle, enabled)) = filter {
            self.check_filter(handle)?;
            if handle.target().table != request.target.table
                || !handle.mode_cached(enabled)
                || handle
                    .display_offset_for_ordinal(enabled, found.parent_ordinal)
                    .is_none()
            {
                return Err(ReplaceError::StaleMatch.into());
            }
        }
        if matches!(&request.scope, FindScope::Field(column) if column != &found.column) {
            return Err(ReplaceError::StaleMatch.into());
        }
        let span = found.span.as_ref().ok_or(ReplaceError::StaleMatch)?;
        if span.start_utf16 > span.end_utf16 {
            return Err(ReplaceError::StaleMatch.into());
        }

        let mut native = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &request.target.table,
            &mut native,
            cancelled,
        )?;
        let mut position = None;
        for (index, field) in schema.columns().iter().enumerate() {
            check(cancelled)?;
            native.charge(
                Resource::Steps,
                1 + field.name().len() as u64 + found.column.len() as u64,
            )?;
            if field.name() == found.column {
                position = Some(index);
                break;
            }
        }
        let position = position.ok_or(ReplaceError::StaleMatch)?;
        let field = &schema.columns()[position];
        let kind = field.scalar_type().ok_or(ReplaceError::UnsupportedField)?;
        if field.complex_type_id().is_some()
            || !matches!(
                kind,
                ScalarType::Text
                    | ScalarType::Byte
                    | ScalarType::Int16
                    | ScalarType::Int32
                    | ScalarType::Int64
            )
            || (request.formatted && kind != ScalarType::Text)
        {
            return Err(ReplaceError::UnsupportedField.into());
        }
        let metadata = Metadata::inspect(
            &mut self.source,
            &request.target.table,
            &mut native,
            cancelled,
        )?;
        let mut output = Presentation::new(cancelled);
        metadata.require_replace_control(&found.column, &mut output)?;
        metadata.require_find_format(
            position,
            &found.column,
            kind,
            request.formatted,
            &mut output,
        )?;
        let mode = matching::query_mode(&mut self.source, &mut native, cancelled)?;
        let order = inspect_database_sort_order(&mut self.source, &mut native, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let policy = match field.text_sort_order() {
            Some(order) => resolver.resolve_native(order)?,
            None => resolver.default_collation(),
        };
        let mut matching = ResourceBudget::new(MATCH_BYTES, 0, MATCH_STEPS);
        let matcher = Matcher::new(request, mode, &mut matching, cancelled)?;
        matcher.require_type(kind)?;

        let image = NativeImage::capture_shared(
            self.edits.image_source.clone(),
            SCOPE,
            &mut native,
            cancelled,
        )?;
        let editor = NativeJournalEditor::new(image, &request.target.table, &mut native, cancelled)
            .map_err(EditError::from)?;
        if editor.update_profile() == UpdateProfile::Unavailable {
            return Err(EditError::Unavailable.into());
        }
        let snapshot = editor.snapshot();
        let index = snapshot
            .columns()
            .iter()
            .position(|column| column.name() == found.column)
            .ok_or(ReplaceError::StaleMatch)?;
        native.charge_allocation(field.name().len() as u64, 1)?;
        if snapshot.columns()[index].type_descriptor() != field.column()?.type_descriptor()
            || filter
                .is_some_and(|(handle, _)| snapshot.rows().len() != handle.source_rows() as usize)
        {
            return Err(ReplaceError::StaleMatch.into());
        }
        let original = snapshot
            .rows()
            .get(found.parent_ordinal as usize)
            .and_then(|row| row.get(index))
            .ok_or(ReplaceError::StaleMatch)?;
        if matches!(original, Value::Null) {
            return Err(ReplaceError::NullMatch.into());
        }
        let mut cell = output.scalar(original, false)?;
        let column = metadata.column(position, &found.column, &mut output)?;
        output.allocation(1, size_of::<Vec<Cell>>() + size_of::<Cell>())?;
        let mut rows = vec![vec![cell]];
        metadata.enrich_projection(&[position], &[column], &mut rows, &mut output)?;
        cell = rows
            .pop()
            .and_then(|mut row| row.pop())
            .ok_or(ReplaceError::StaleMatch)?;
        let text = matcher.text(original, &mut cell, &mut output)?;
        let range = byte_range(text, span, &mut matching, cancelled)?;
        let actual = match matcher.special(original) {
            Some(true) => Some(FindSpan {
                start_utf16: 0,
                end_utf16: 0,
            }),
            Some(false) => None,
            None => matcher.find(
                text,
                match request.direction {
                    FindDirection::Up => StringSearch::ReverseThrough(span.end_utf16 as usize),
                    FindDirection::All | FindDirection::Down => {
                        StringSearch::ForwardFrom(span.start_utf16 as usize)
                    }
                },
                policy,
                &mut matching,
                cancelled,
            )?,
        };
        if actual.as_ref() != Some(span) {
            return Err(ReplaceError::StaleMatch.into());
        }
        let input = EditInput {
            text: splice(text, range, replacement, &mut output)?,
            is_null: false,
        };
        input::parse_original(TypeDescriptor::new(kind), &input, original, &mut output)?;
        let target = EditTarget {
            document_revision: request.target.document_revision,
            table: output.text(&request.target.table)?,
            column: output.text(&found.column)?,
            parent_ordinal: found.parent_ordinal,
        };
        drop(editor);
        let Response::EditDraft(mut draft) = self.begin_edit(&target, cancelled)? else {
            return Err(ReplaceError::StaleMatch.into());
        };
        draft.initial = input;
        Ok(Response::EditDraft(draft))
    }
}

fn byte_range(
    text: &str,
    span: &FindSpan,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Range<usize>, Error> {
    let mut units = 0_u32;
    let mut start = None;
    let mut end = None;
    for (byte, character) in text.char_indices() {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        if units == span.start_utf16 {
            start = Some(byte);
        }
        if units == span.end_utf16 {
            end = Some(byte);
        }
        units = units
            .checked_add(character.len_utf16() as u32)
            .ok_or(ReplaceError::StaleMatch)?;
    }
    if units == span.start_utf16 {
        start = Some(text.len());
    }
    if units == span.end_utf16 {
        end = Some(text.len());
    }
    Ok(start.ok_or(ReplaceError::StaleMatch)?..end.ok_or(ReplaceError::StaleMatch)?)
}

fn splice(
    text: &str,
    range: Range<usize>,
    replacement: &str,
    output: &mut Presentation<'_>,
) -> Result<String, Error> {
    let length = text
        .len()
        .checked_sub(range.len())
        .and_then(|length| length.checked_add(replacement.len()))
        .ok_or(EditError::InputLimit)?;
    if length > MAX_INPUT_BYTES {
        return Err(EditError::InputLimit.into());
    }
    output.allocation(length, 1)?;
    output.work(length as u64)?;
    let mut value = String::new();
    value.try_reserve_exact(length)?;
    value.push_str(&text[..range.start]);
    value.push_str(replacement);
    value.push_str(&text[range.end..]);
    Ok(value)
}
