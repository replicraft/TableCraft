//! Raw QBE authoring text is retained data; unresolved text grants no Build authority.
mod state;

use crate::workspace::objects::memory::Counter;
use std::sync::{Arc, atomic::AtomicUsize};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, MAX_DESIGN_TEXT_BYTES, QueryDesignDraft,
    QueryDesignError, draft_text_bytes,
};

// Charge both ownership counters and a full payload-alignment unit, as with the
// existing shared draft envelope. Payload structs and buffers are charged below.
const TEXT_CONTROL_BYTES: usize = size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<String>();
const FIELD_CONTROL_BYTES: usize =
    size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<OriginalField>();

#[derive(Default)]
pub(super) struct FieldInputs {
    entries: Vec<FieldInput>,
    pub(super) reveal: Option<usize>,
}

struct FieldInput {
    column: usize,
    text: Arc<String>,
    baseline: Arc<OriginalField>,
    complete: bool,
    source_hint: Option<u64>,
}

struct OriginalField {
    expression: DesignExpression,
    alias: Option<String>,
}

impl FieldInputs {
    pub(super) fn text(&self, column: usize) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.column == column)
            .map(|entry| entry.text.as_str())
    }

    pub(super) fn edit_cost(
        &self,
        index: usize,
        column: &DesignColumn,
    ) -> Result<usize, QueryDesignError> {
        if let Some(entry) = self.entries.iter().find(|entry| entry.column == index) {
            return Ok(entry
                .text
                .len()
                .max(entry.baseline.payload_bytes()?)
                .max(payload_bytes(column)?));
        }
        payload_bytes(column)
    }

    pub(super) fn source_hint(&self, column: usize) -> Option<u64> {
        self.entries
            .iter()
            .find(|entry| entry.column == column)?
            .source_hint
    }

    pub(super) fn has(&self, column: usize) -> bool {
        self.entries.iter().any(|entry| entry.column == column)
    }

    pub(super) fn clear_committed(&mut self) {
        self.entries.clear();
        self.reveal = None;
    }

    pub(super) fn editable_bytes(
        &self,
        draft: &QueryDesignDraft,
    ) -> Result<usize, QueryDesignError> {
        let mut total = draft_text_bytes(draft)?;
        for entry in &self.entries {
            let column = draft
                .columns
                .get(entry.column)
                .ok_or(QueryDesignError::InvalidReference)?;
            // Reserve explicit Cancel as well as current authoring text, so
            // shortening one field cannot fund edits that make Cancel unsafe.
            let reserved = entry
                .text
                .len()
                .max(entry.baseline.payload_bytes()?)
                .max(payload_bytes(column)?);
            total = total
                .checked_sub(payload_bytes(column)?)
                .and_then(|bytes| bytes.checked_add(reserved))
                .ok_or(QueryDesignError::Limit)?;
        }
        if total > MAX_DESIGN_TEXT_BYTES {
            return Err(QueryDesignError::Limit);
        }
        Ok(total)
    }

    pub(super) fn unresolved(&self) -> bool {
        self.entries.iter().any(|entry| !entry.complete)
    }

    pub(super) fn first_unresolved(&self) -> Option<usize> {
        self.entries
            .iter()
            .find(|entry| !entry.complete)
            .map(|entry| entry.column)
    }

    pub(super) fn record(
        &mut self,
        index: usize,
        column: &DesignColumn,
        text: String,
        complete: bool,
        source_hint: Option<u64>,
    ) -> Result<(), QueryDesignError> {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.column == index) {
            entry.text = Arc::new(text);
            entry.complete = complete;
            entry.source_hint = source_hint;
            return Ok(());
        }
        let baseline = OriginalField::copy(column)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| QueryDesignError::Limit)?;
        self.entries.push(FieldInput {
            column: index,
            text: Arc::new(text),
            baseline: Arc::new(baseline),
            complete,
            source_hint,
        });
        Ok(())
    }

    pub(super) fn blur(&mut self, column: usize) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.column == column) {
            entry.source_hint = None;
        }
    }

    pub(super) fn cancel(
        &mut self,
        index: usize,
        column: &mut DesignColumn,
    ) -> Result<bool, QueryDesignError> {
        let Some(position) = self.entries.iter().position(|entry| entry.column == index) else {
            return Ok(false);
        };
        let entry = self.entries.remove(position);
        let original = match Arc::try_unwrap(entry.baseline) {
            Ok(original) => original,
            Err(shared) => {
                match OriginalField::copy_parts(&shared.expression, shared.alias.as_deref()) {
                    Ok(original) => original,
                    Err(error) => {
                        self.entries.insert(
                            position,
                            FieldInput {
                                baseline: shared,
                                ..entry
                            },
                        );
                        return Err(error);
                    }
                }
            }
        };
        column.expression = original.expression;
        column.alias = original.alias;
        Ok(true)
    }

    pub(super) fn remove_column(&mut self, column: usize) {
        self.entries.retain(|entry| entry.column != column);
        for entry in &mut self.entries {
            if entry.column > column {
                entry.column -= 1;
            }
        }
        self.reveal = None;
    }

    pub(super) fn clear_hints(&mut self) {
        for entry in &mut self.entries {
            entry.source_hint = None;
        }
        self.reveal = None;
    }

    pub(super) fn snapshot(&self) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .map(|entry| FieldInput {
                    column: entry.column,
                    // NO_CLONE: Source refresh/parking retains a separate immutable
                    // user-data snapshot; sharing avoids copying its raw text bytes.
                    text: Arc::clone(&entry.text),
                    // NO_CLONE: Explicit Cancel needs the same immutable baseline
                    // independently in both retained and freshly restored layouts.
                    baseline: Arc::clone(&entry.baseline),
                    complete: entry.complete,
                    source_hint: None,
                })
                .collect(),
            reveal: None,
        }
    }

    pub(super) fn count(&self, count: &mut Counter) {
        count.vector(&self.entries);
        for entry in &self.entries {
            count.add(TEXT_CONTROL_BYTES + size_of::<String>());
            count.string(&entry.text);
            count.add(FIELD_CONTROL_BYTES + size_of::<OriginalField>());
            expression_bytes(count, &entry.baseline.expression);
            count.optional(&entry.baseline.alias);
        }
    }
}

impl OriginalField {
    fn payload_bytes(&self) -> Result<usize, QueryDesignError> {
        let operand = match &self.expression {
            DesignExpression::Field(field) => field.column.len(),
            DesignExpression::Expression(text) => text.len(),
            DesignExpression::AllFields(_) => 0,
        };
        operand
            .checked_add(self.alias.as_ref().map_or(0, String::len))
            .ok_or(QueryDesignError::Limit)
    }

    fn copy(column: &DesignColumn) -> Result<Self, QueryDesignError> {
        Self::copy_parts(&column.expression, column.alias.as_deref())
    }

    fn copy_parts(
        expression: &DesignExpression,
        alias: Option<&str>,
    ) -> Result<Self, QueryDesignError> {
        // NO_CLONE: A mutable parsed column and its explicit-Cancel baseline
        // have independent lifetimes. Shared immutable baseline storage then
        // survives source refresh; the editable public model cannot borrow it.
        let expression = match expression {
            DesignExpression::Field(field) => DesignExpression::Field(DesignField {
                source_id: field.source_id,
                column: copy_text(&field.column)?,
            }),
            DesignExpression::Expression(text) => DesignExpression::Expression(copy_text(text)?),
            DesignExpression::AllFields(source) => DesignExpression::AllFields(*source),
        };
        Ok(Self {
            expression,
            alias: alias.map(copy_text).transpose()?,
        })
    }
}

pub(super) fn copy_text(value: &str) -> Result<String, QueryDesignError> {
    let mut text = String::new();
    text.try_reserve_exact(value.len())
        .map_err(|_| QueryDesignError::Limit)?;
    text.push_str(value);
    Ok(text)
}

fn expression_bytes(count: &mut Counter, expression: &DesignExpression) {
    match expression {
        DesignExpression::Field(field) => count.string(&field.column),
        DesignExpression::Expression(text) => count.string(text),
        DesignExpression::AllFields(_) => {}
    }
}

pub(super) fn payload_bytes(column: &DesignColumn) -> Result<usize, QueryDesignError> {
    let operand = match &column.expression {
        DesignExpression::Field(field) => field.column.len(),
        DesignExpression::Expression(text) => text.len(),
        DesignExpression::AllFields(_) => 0,
    };
    operand
        .checked_add(column.alias.as_ref().map_or(0, String::len))
        .ok_or(QueryDesignError::Limit)
}

#[cfg(test)]
mod tests;
