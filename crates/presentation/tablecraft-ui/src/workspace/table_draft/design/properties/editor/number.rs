//! Joint validation retains incomplete precision/scale text without changing the declaration.
use super::{Editors, MAX_SIZE_INPUT_BYTES};
use tablecraft_builders::table::{DraftDefaultMode, DraftNumberSize, TableDraft};
use tablecraft_schema::DecimalConstraints;

pub(super) struct DecimalInput {
    precision: String,
    scale: String,
    pub(super) invalid: bool,
}

impl DecimalInput {
    pub(super) fn new(constraints: DecimalConstraints) -> Self {
        Self {
            precision: constraints.precision().to_string(),
            scale: constraints.scale().to_string(),
            invalid: false,
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.precision.capacity() + self.scale.capacity()
    }
}

impl Editors {
    pub(in crate::workspace::table_draft::design::properties) fn decimal_text(
        &self,
        position: usize,
        precision: bool,
    ) -> &str {
        let Some(input) = &self.entries[position].decimal else {
            return "";
        };
        if precision {
            &input.precision
        } else {
            &input.scale
        }
    }

    pub(in crate::workspace::table_draft::design::properties) fn decimal_rejected(
        &self,
        position: usize,
    ) -> bool {
        self.entries[position]
            .decimal
            .as_ref()
            .is_some_and(|input| input.invalid)
    }

    pub(in crate::workspace::table_draft::design::properties) fn set_number_size(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        size: DraftNumberSize,
    ) -> bool {
        if self.decimal_rejected(position) {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let field = self.entries[position].field;
        if draft.set_field_number_size(field, size).is_err() {
            return false;
        }
        self.entries[position].unsupported_default = size == DraftNumberSize::ReplicationId
            && draft
                .fields()
                .iter()
                .find(|candidate| candidate.id() == field)
                .is_some_and(|candidate| {
                    matches!(candidate.default_mode(), DraftDefaultMode::Literal(_))
                });
        self.revision = revision;
        true
    }

    pub(in crate::workspace::table_draft::design::properties) fn set_decimal_text(
        &mut self,
        draft: &mut TableDraft,
        position: usize,
        precision: bool,
        raw: String,
        rejected: bool,
    ) -> bool {
        let field = self.entries[position].field;
        if !draft.fields().iter().any(|candidate| {
            candidate.id() == field
                && !candidate.is_primary()
                && candidate.decimal_constraints().is_some()
        }) {
            return false;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let Some(input) = &mut self.entries[position].decimal else {
            return false;
        };
        let (next_precision, next_scale) = if precision {
            (raw.as_str(), input.scale.as_str())
        } else {
            (input.precision.as_str(), raw.as_str())
        };
        let parsed = next_precision
            .parse::<u8>()
            .ok()
            .zip(next_scale.parse::<u8>().ok());
        let constraints = parsed.and_then(|(p, s)| DecimalConstraints::new(p, s).ok());
        let valid = !rejected && raw.len() <= MAX_SIZE_INPUT_BYTES && constraints.is_some();
        if valid
            && draft
                .set_field_decimal_constraints(field, constraints.unwrap())
                .is_err()
        {
            return false;
        }
        if !rejected && raw.len() <= MAX_SIZE_INPUT_BYTES {
            if precision {
                input.precision = raw;
            } else {
                input.scale = raw;
            }
        }
        input.invalid = !valid;
        self.revision = revision;
        valid
    }
}
