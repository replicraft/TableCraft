//! Raw property editors are separate from validated declarations and native input parsing.
mod number;

use number::DecimalInput;
use tablecraft_builders::table::{
    DraftDefaultMode, DraftFieldId, DraftFieldType, DraftNumberSize, MAX_DRAFT_DEFAULT_BYTES,
    MIN_SHORT_TEXT_UNITS, SHORT_TEXT_UNITS, TableDraft,
};

pub(super) const MAX_SIZE_INPUT_BYTES: usize = 64;

#[derive(Default)]
pub(in crate::workspace::table_draft) struct Editors {
    entries: Vec<Entry>,
    revision: u64,
}

struct Entry {
    field: DraftFieldId,
    size: Option<String>,
    decimal: Option<DecimalInput>,
    literal: String,
    size_invalid: bool,
    literal_rejected: bool,
    unsupported_default: bool,
}

impl Entry {
    fn rejected(&self) -> bool {
        self.size_invalid
            || self.literal_rejected
            || self.unsupported_default
            || self.decimal.as_ref().is_some_and(|input| input.invalid)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DefaultChoice {
    TypeDefault,
    Absent,
    Null,
    Literal,
}

impl DefaultChoice {
    pub(super) fn from_mode(mode: &DraftDefaultMode) -> Self {
        match mode {
            DraftDefaultMode::TypeDefault => Self::TypeDefault,
            DraftDefaultMode::Absent => Self::Absent,
            DraftDefaultMode::Null => Self::Null,
            DraftDefaultMode::Literal(_) => Self::Literal,
        }
    }

    pub(super) fn key(self) -> &'static str {
        match self {
            Self::TypeDefault => "table-draft-property-type-default",
            Self::Absent => "table-draft-property-absent",
            Self::Null => "table-draft-property-null",
            Self::Literal => "table-draft-property-literal",
        }
    }
}

impl Editors {
    pub(in crate::workspace::table_draft) fn raw_edit_available(&self) -> bool {
        self.revision.checked_add(1).is_some()
    }

    pub(in crate::workspace::table_draft) fn record_raw_edit(&mut self) -> bool {
        // The widget preflights this revision before receiving any input.
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        self.revision = revision;
        true
    }

    pub(in crate::workspace::table_draft) fn revision(&self) -> u64 {
        self.revision
    }

    pub(in crate::workspace::table_draft) fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.entries.capacity() * size_of::<Entry>()
            + self
                .entries
                .iter()
                .map(|entry| {
                    entry.literal.capacity()
                        + entry.size.as_ref().map_or(0, String::capacity)
                        + entry
                            .decimal
                            .as_ref()
                            .map_or(0, DecimalInput::retained_bytes)
                })
                .sum::<usize>()
    }

    pub(in crate::workspace::table_draft) fn rejected(&self) -> bool {
        self.entries.iter().any(Entry::rejected)
    }

    pub(in crate::workspace::table_draft) fn field_rejected(&self, field: DraftFieldId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.field == field && entry.rejected())
    }

    pub(in crate::workspace::table_draft) fn ensure(
        &mut self,
        draft: &TableDraft,
        id: DraftFieldId,
    ) -> Option<usize> {
        let field = draft.fields().iter().find(|field| field.id() == id)?;
        let unsupported_default = field.number_size() == Some(DraftNumberSize::ReplicationId)
            && matches!(field.default_mode(), DraftDefaultMode::Literal(_));
        if let Some(position) = self.entries.iter().position(|entry| entry.field == id) {
            self.entries[position].unsupported_default = unsupported_default;
            if self.entries[position].size.is_none() && field.kind() == DraftFieldType::ShortText {
                self.entries[position].size = Some(field.short_text_units()?.to_string());
            }
            if self.entries[position].decimal.is_none() {
                self.entries[position].decimal = field.decimal_constraints().map(DecimalInput::new);
            }
            return Some(position);
        }
        self.entries.try_reserve(1).ok()?;
        let literal = match field.default_mode() {
            // NO_CLONE: Default editor cache and validated TableDraft each own a literal
            // independently; setter refusal must retain the original raw edit and
            // success keeps both caches synchronized.
            DraftDefaultMode::Literal(text) => text.clone(),
            _ => String::new(),
        };
        self.entries.push(Entry {
            field: id,
            size: field.short_text_units().map(|size| size.to_string()),
            decimal: field.decimal_constraints().map(DecimalInput::new),
            literal,
            size_invalid: false,
            literal_rejected: false,
            unsupported_default,
        });
        Some(self.entries.len() - 1)
    }

    pub(super) fn size(&self, position: usize) -> &str {
        self.entries[position].size.as_deref().unwrap_or("")
    }

    pub(super) fn literal(&self, position: usize) -> &str {
        &self.entries[position].literal
    }

    pub(super) fn size_rejected(&self, position: usize) -> bool {
        self.entries[position].size_invalid
    }

    pub(super) fn literal_rejected(&self, position: usize) -> bool {
        self.entries[position].literal_rejected
    }

    pub(in crate::workspace::table_draft) fn set_size(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        raw: String,
        rejected: bool,
    ) -> bool {
        let field = self.entries[position].field;
        if !draft.fields().iter().any(|candidate| {
            candidate.id() == field
                && !candidate.is_protected()
                && candidate.kind() == DraftFieldType::ShortText
        }) {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let parsed = raw
            .parse::<u16>()
            .ok()
            .filter(|size| (MIN_SHORT_TEXT_UNITS..=SHORT_TEXT_UNITS).contains(size));
        let valid = !rejected && raw.len() <= MAX_SIZE_INPUT_BYTES && parsed.is_some();
        if valid && draft.set_field_text_units(field, parsed.unwrap()).is_err() {
            return false;
        }
        if !rejected && raw.len() <= MAX_SIZE_INPUT_BYTES {
            self.entries[position].size = Some(raw);
        }
        self.entries[position].size_invalid = !valid;
        self.revision = revision;
        valid
    }

    pub(super) fn set_required(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        value: Option<bool>,
    ) -> bool {
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        if draft
            .set_field_required(self.entries[position].field, value)
            .is_err()
        {
            return false;
        }
        self.revision = revision;
        true
    }

    pub(super) fn set_zero_length(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        value: Option<bool>,
    ) -> bool {
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        if draft
            .set_field_allow_zero_length(self.entries[position].field, value)
            .is_err()
        {
            return false;
        }
        self.revision = revision;
        true
    }

    pub(super) fn set_mode(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        choice: DefaultChoice,
    ) -> bool {
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let mode = match choice {
            DefaultChoice::TypeDefault => DraftDefaultMode::TypeDefault,
            DefaultChoice::Absent => DraftDefaultMode::Absent,
            DefaultChoice::Null => DraftDefaultMode::Null,
            DefaultChoice::Literal => {
                // NO_CLONE: Default editor cache and validated TableDraft each own a literal
                // independently; setter refusal must retain the original raw edit and
                // success keeps both caches synchronized.
                DraftDefaultMode::Literal(self.entries[position].literal.clone())
            }
        };
        if draft
            .set_field_default(self.entries[position].field, mode)
            .is_err()
        {
            return false;
        }
        self.entries[position].literal_rejected = false;
        self.entries[position].unsupported_default = choice == DefaultChoice::Literal
            && draft
                .fields()
                .iter()
                .find(|field| field.id() == self.entries[position].field)
                .is_some_and(|field| field.number_size() == Some(DraftNumberSize::ReplicationId));
        self.revision = revision;
        true
    }

    pub(super) fn set_literal(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        raw: String,
        rejected: bool,
    ) -> bool {
        let field = self.entries[position].field;
        if !draft
            .fields()
            .iter()
            .any(|candidate| candidate.id() == field && !candidate.is_protected())
        {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        if rejected || raw.len() > MAX_DRAFT_DEFAULT_BYTES {
            self.entries[position].literal_rejected = true;
            self.revision = revision;
            return false;
        }
        // NO_CLONE: Default editor cache and validated TableDraft each own a literal
        // independently; setter refusal must retain the original raw edit and success
        // keeps both caches synchronized.
        if draft
            .set_field_default(
                self.entries[position].field,
                DraftDefaultMode::Literal(raw.clone()),
            )
            .is_err()
        {
            return false;
        }
        self.entries[position].unsupported_default = draft
            .fields()
            .iter()
            .find(|candidate| candidate.id() == field)
            .is_some_and(|candidate| {
                candidate.number_size() == Some(DraftNumberSize::ReplicationId)
            });
        self.entries[position].literal = raw;
        self.entries[position].literal_rejected = false;
        self.revision = revision;
        true
    }

    pub(in crate::workspace::table_draft) fn removal(
        &self,
        field: DraftFieldId,
    ) -> Option<(Option<usize>, u64)> {
        Some((
            self.entries.iter().position(|entry| entry.field == field),
            self.revision.checked_add(1)?,
        ))
    }

    pub(in crate::workspace::table_draft) fn remove_prepared(
        &mut self,
        position: Option<usize>,
        revision: u64,
    ) {
        if let Some(position) = position {
            self.entries.remove(position);
        }
        self.revision = revision;
    }
}

#[cfg(test)]
mod tests;
