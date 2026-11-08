//! Transfer retained original inputs into an actual runtime-issued insertion draft.
use super::{
    Choice, Dialog, FieldInput, InsertInput, inputs_match, text_bytes_fit, within_input_limits,
};
use crate::{InitialRecord, InputHint, workspace::State};
use tablecraft_builders::table::{DraftFieldType, DraftNumberSize, SnapshotField};

impl State {
    /// Seed a pristine, source-current native insertion draft after first-table save.
    /// This borrows the retained inputs: failure never consumes their owner and
    /// success grants no review, generated value, row handle or write authority.
    pub fn seed_initial_insert(&mut self, draft_id: u64, record: &InitialRecord) -> bool {
        let snapshot = &record.snapshot;
        let inputs = &record.inputs;
        let Some(Dialog::Draft {
            draft,
            entries,
            pending: None,
            review: None,
            error: None,
            inline,
            initial_seeded,
            ..
        }) = &self.insertion
        else {
            return false;
        };
        if draft.id != draft_id
            || !self.insert_draft_current(draft)
            || draft.target.table != snapshot.table_name()
            || draft.fields.len() != snapshot.fields().len()
            || inputs.len() != draft.fields.len()
            || record.raw_text.len() != inputs.len()
            || !text_bytes_fit(record.raw_text.iter().map(String::as_str))
            || !within_input_limits(inputs)
            || !inline.can_seed_initial()
        {
            return false;
        }
        for (position, ((native, original), input)) in draft
            .fields
            .iter()
            .zip(snapshot.fields())
            .zip(inputs)
            .enumerate()
        {
            let raw = &record.raw_text[position];
            let generated = original.kind() == DraftFieldType::AutoNumberIncrement;
            if native.name != original.name()
                || Some(native.input_hint) != hint(original)
                || native.auto_number != generated
                || original.is_protected() != generated
                || generated && (!original.is_primary() || position != 0)
                || generated && (!matches!(input, InsertInput::Omitted) || !raw.is_empty())
                || matches!(input, InsertInput::Value(value) if &value.text != raw)
            {
                return false;
            }
        }
        if *initial_seeded {
            return inputs_match(&draft.fields, entries, inputs)
                && entries
                    .iter()
                    .zip(&record.raw_text)
                    .all(|(entry, raw)| &entry.text == raw);
        }
        if entries.iter().any(|entry| entry != &FieldInput::default()) {
            return false;
        }
        let mut retained = Vec::new();
        if retained.try_reserve_exact(inputs.len()).is_err() {
            return false;
        }
        for (input, source) in inputs.iter().zip(&record.raw_text) {
            let choice = match input {
                InsertInput::Omitted => Choice::Omitted,
                InsertInput::Value(value) if value.is_null => Choice::Null,
                InsertInput::Value(_) => Choice::Value,
            };
            let mut text = String::new();
            if text.try_reserve_exact(source.len()).is_err() {
                return false;
            }
            text.push_str(source);
            retained.push(FieldInput {
                choice,
                text,
                input_rejected: false,
            });
        }
        let Some(Dialog::Draft {
            entries,
            initial_seeded,
            ..
        }) = &mut self.insertion
        else {
            return false;
        };
        *entries = retained;
        *initial_seeded = true;
        true
    }
}

fn hint(field: &SnapshotField) -> Option<InputHint> {
    Some(match field.kind() {
        DraftFieldType::AutoNumberIncrement => InputHint::Int32,
        DraftFieldType::Number => match field.number_size()? {
            DraftNumberSize::Byte => InputHint::Byte,
            DraftNumberSize::Integer => InputHint::Int16,
            DraftNumberSize::LongInteger => InputHint::Int32,
            DraftNumberSize::Single => InputHint::Single,
            DraftNumberSize::Double => InputHint::Double,
            DraftNumberSize::Decimal => InputHint::Decimal,
            DraftNumberSize::ReplicationId => InputHint::Guid,
        },
        DraftFieldType::ShortText => InputHint::Text,
        DraftFieldType::LargeInteger => InputHint::Int64,
        DraftFieldType::YesNo => InputHint::Boolean,
        DraftFieldType::Currency => InputHint::Currency,
        DraftFieldType::DateTime => InputHint::DateTimeIso,
    })
}

#[cfg(test)]
mod tests;
