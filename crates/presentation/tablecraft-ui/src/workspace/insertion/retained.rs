//! Exact insertion input retained independently of an expired native capability.
use super::{Dialog, FieldInput, InsertDraft, InsertTarget, State, entries_within_limits};
use crate::localization::tr;
use crate::workspace::objects::memory::Counter;
use egui::Ui;

const COPY_ROWS: usize = 2;
const PREVIEW_SCALARS: usize = 512;

/// Host-supplied native outcome evidence, never inferred from display messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertRetryPermission {
    Blocked,
    OriginalImage,
}

pub(in crate::workspace) struct RetainedInput {
    pub(super) draft: Box<InsertDraft>,
    pub(super) entries: Vec<FieldInput>,
    pub(super) initial_notice: Option<InsertTarget>,
}

impl RetainedInput {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.add(size_of::<InsertDraft>());
        count.string(&self.draft.target.table);
        count.vector(&self.draft.fields);
        for field in &self.draft.fields {
            count.string(&field.name);
        }
        count.vector(&self.entries);
        for entry in &self.entries {
            count.string(&entry.text);
        }
        if let Some(target) = &self.initial_notice {
            count.string(&target.table);
        }
        count.finish()
    }

    pub(super) fn show(&self, ui: &mut Ui) {
        ui.separator();
        ui.label(tr(ui, "insertion-retained-help"));
        if self.initial_notice.is_some() {
            ui.label(tr(ui, "insertion-retained-schema-help"));
        }
        for (field, entry) in self.draft.fields.iter().zip(&self.entries) {
            ui.push_id(("retained_insert", &field.name), |ui| {
                let label = ui.label(&field.name);
                if field.auto_number {
                    ui.label(tr(ui, "insertion-generated-value"));
                    return;
                }
                let mode = match entry.choice {
                    super::Choice::Omitted => "insertion-omitted",
                    super::Choice::Null => "insertion-null",
                    super::Choice::Value => "insertion-value",
                };
                ui.small(tr(ui, mode));
                // Bound shaping work, while the explicit Copy action retains
                // all original text, including text hidden by Omitted or NULL.
                let end = entry
                    .text
                    .char_indices()
                    .nth(PREVIEW_SCALARS)
                    .map_or(entry.text.len(), |(index, _)| index);
                let mut text = &entry.text[..end];
                let response = ui
                    .add(egui::TextEdit::multiline(&mut text).desired_rows(COPY_ROWS))
                    .labelled_by(label.id);
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_read_only();
                    node.remove_action(egui::accesskit::Action::SetValue);
                    node.remove_action(egui::accesskit::Action::ReplaceSelectedText);
                });
                if end < entry.text.len() {
                    ui.small(tr(ui, "insertion-preview-truncated"));
                }
                if ui.button(tr(ui, "insertion-copy-input")).clicked() {
                    // The clipboard output owns its text beyond this borrowed cell/retained
                    // editor; the existing source text must remain available.
                    ui.ctx().copy_text(entry.text.clone());
                }
            });
        }
    }
}

impl State {
    /// Synthetic presentation fixture only; never produces a native capability.
    pub(in crate::workspace) fn prepare_retained_insert_preview(
        &mut self,
    ) -> Option<(u64, Vec<super::InsertInput>)> {
        if !self.mock_preview {
            return None;
        }
        let Some(Dialog::Draft {
            draft,
            entries,
            pending: None,
            review: None,
            ..
        }) = &mut self.insertion
        else {
            return None;
        };
        if entries.len() != 3 || draft.fields.iter().any(|field| field.auto_number) {
            return None;
        }
        entries[0] = FieldInput {
            choice: super::Choice::Value,
            text: " 0007 ".into(),
            input_rejected: false,
        };
        entries[1] = FieldInput {
            choice: super::Choice::Omitted,
            text: "Original hidden input".into(),
            input_rejected: false,
        };
        entries[2] = FieldInput {
            choice: super::Choice::Null,
            text: "Retained text behind NULL".into(),
            input_rejected: false,
        };
        Some((draft.id, super::inputs(entries)))
    }

    pub(super) fn seed_retained_insert(&mut self, draft_id: u64, retained: &RetainedInput) -> bool {
        let Some(Dialog::Draft {
            draft,
            entries,
            pending: None,
            review: None,
            error: None,
            inline,
            ..
        }) = &self.insertion
        else {
            return false;
        };
        if draft.id != draft_id
            || !self.insert_draft_current(draft)
            || draft.target.table != retained.draft.target.table
            || draft.fields.len() != retained.draft.fields.len()
            || !inline.can_seed_initial()
            || !entries_within_limits(&retained.entries)
            || entries.iter().any(|entry| entry != &FieldInput::default())
            || !draft
                .fields
                .iter()
                .zip(&retained.draft.fields)
                .all(|(fresh, old)| {
                    fresh.name == old.name
                        && fresh.input_hint == old.input_hint
                        && fresh.nullable == old.nullable
                        && fresh.auto_number == old.auto_number
                })
        {
            return false;
        }
        let mut replacement = Vec::new();
        if replacement
            .try_reserve_exact(retained.entries.len())
            .is_err()
        {
            return false;
        }
        for entry in &retained.entries {
            let mut text = String::new();
            if text.try_reserve_exact(entry.text.len()).is_err() {
                return false;
            }
            text.push_str(&entry.text);
            replacement.push(FieldInput {
                choice: entry.choice,
                text,
                input_rejected: entry.input_rejected,
            });
        }
        let Some(Dialog::Draft { entries, .. }) = &mut self.insertion else {
            return false;
        };
        *entries = replacement;
        true
    }
}

#[cfg(test)]
mod tests;
