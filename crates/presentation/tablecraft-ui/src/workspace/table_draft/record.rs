//! One unsaved record's raw intent, without native row identity or generated values.
use crate::localization::{message, tr};
use crate::workspace::{EditInput, InsertInput};
use egui::{Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::{
    DraftField, DraftFieldId, DraftFieldType, TableDraft, TableDraftSnapshot,
};

const MAX_RECORD_TEXT_BYTES: usize = 8 * 1024 * 1024;
const INPUT_MODE_WIDTH: f32 = 20.0;
const MIN_INPUT_WIDTH: f32 = 24.0;
const MODE_CHEVRON_HALF_WIDTH: f32 = 3.0;
const MODE_CHEVRON_HALF_HEIGHT: f32 = 1.5;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Choice {
    #[default]
    Omitted,
    Null,
    Value,
}

#[derive(Clone)]
struct Entry {
    id: DraftFieldId,
    kind: DraftFieldType,
    choice: Choice,
    text: String,
    rejected: bool,
}

/// Raw editing state is keyed to declaration identity, never a displayed name.
#[derive(Default)]
pub(super) struct RowDraft {
    entries: Vec<Entry>,
    revision: u64,
}

impl RowDraft {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Synchronize only declarations. Adding a field never supplies a row value.
    pub(super) fn synchronize(&mut self, draft: &TableDraft) -> bool {
        if self.entries.len() == draft.fields().len()
            && self
                .entries
                .iter()
                .zip(draft.fields())
                .all(|(entry, field)| entry.id == field.id() && entry.kind == field.kind())
        {
            return true;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        let mut entries = Vec::new();
        if entries.try_reserve_exact(draft.fields().len()).is_err() {
            return false;
        }
        for field in draft.fields() {
            let retained = self
                .entries
                .iter()
                .find(|entry| entry.id == field.id() && entry.kind == field.kind());
            entries.push(retained.cloned().unwrap_or_else(|| Entry {
                id: field.id(),
                kind: field.kind(),
                choice: Choice::Omitted,
                text: String::new(),
                rejected: false,
            }));
        }
        self.entries = entries;
        self.revision = revision;
        true
    }

    /// Preflight removal without cloning or discarding any retained input.
    pub(super) fn removal(&self, id: DraftFieldId) -> Option<(usize, u64)> {
        let position = self.entries.iter().position(|entry| entry.id == id)?;
        if self.entries[position].kind == DraftFieldType::AutoNumberIncrement {
            return None;
        }
        Some((position, self.revision.checked_add(1)?))
    }

    /// Called only after both schema and row preflights have succeeded.
    pub(super) fn remove_prepared(&mut self, position: usize, revision: u64) {
        self.entries.remove(position);
        self.revision = revision;
    }

    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn is_modified(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.choice != Choice::Omitted || !entry.text.is_empty())
    }

    pub(super) fn has_rejected_input(&self) -> bool {
        self.entries.iter().any(|entry| entry.rejected)
    }

    fn reject(&mut self, position: usize) {
        if !self.entries[position].rejected
            && let Some(revision) = self.revision.checked_add(1)
        {
            self.entries[position].rejected = true;
            self.revision = revision;
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.entries.capacity() * size_of::<Entry>()
            + self
                .entries
                .iter()
                .map(|entry| entry.text.capacity())
                .sum::<usize>()
    }

    /// Refuse oversized edits whole. Omitted keeps raw text for later restoration.
    pub(super) fn set_input(&mut self, id: DraftFieldId, input: InsertInput) -> bool {
        let Some(position) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        if self.entries[position].kind == DraftFieldType::AutoNumberIncrement {
            return matches!(input, InsertInput::Omitted);
        }
        let (choice, text) = match input {
            InsertInput::Omitted => (Choice::Omitted, self.entries[position].text.clone()),
            InsertInput::Value(value) => {
                if value.is_null && self.entries[position].kind == DraftFieldType::YesNo {
                    self.reject(position);
                    return false;
                }
                (
                    if value.is_null {
                        Choice::Null
                    } else {
                        Choice::Value
                    },
                    value.text,
                )
            }
        };
        let other_bytes = self
            .entries
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != position)
            .map(|(_, entry)| entry.text.len())
            .sum::<usize>();
        if text.len() > MAX_RECORD_TEXT_BYTES.saturating_sub(other_bytes)
            || text.chars().count() > crate::text_input::MAX_EDITOR_SCALARS
        {
            self.reject(position);
            return false;
        }
        let entry = &self.entries[position];
        if entry.choice == choice && entry.text == text && !entry.rejected {
            return true;
        }
        let Some(revision) = self.revision.checked_add(1) else {
            return false;
        };
        self.entries[position].choice = choice;
        self.entries[position].text = text;
        self.entries[position].rejected = false;
        self.revision = revision;
        true
    }

    fn matches_snapshot(&self, snapshot: &TableDraftSnapshot) -> bool {
        !self.has_rejected_input()
            && self.entries.len() == snapshot.fields().len()
            && self
                .entries
                .iter()
                .zip(snapshot.fields())
                .all(|(entry, field)| {
                    entry.id == field.id()
                        && entry.kind == field.kind()
                        && (field.kind() != DraftFieldType::AutoNumberIncrement
                            || entry.choice == Choice::Omitted && entry.text.is_empty())
                })
    }

    pub(super) fn inputs(&self, snapshot: &TableDraftSnapshot) -> Option<Vec<InsertInput>> {
        if !self.matches_snapshot(snapshot) {
            return None;
        }
        Some(
            self.entries
                .iter()
                .map(|entry| match entry.choice {
                    Choice::Omitted => InsertInput::Omitted,
                    Choice::Null | Choice::Value => InsertInput::Value(EditInput {
                        text: entry.text.clone(),
                        is_null: entry.choice == Choice::Null,
                    }),
                })
                .collect(),
        )
    }

    /// Presentation cache only: omitted raw text never becomes supplied data.
    pub(super) fn raw_texts(&self, snapshot: &TableDraftSnapshot) -> Option<Vec<String>> {
        if !self.matches_snapshot(snapshot) {
            return None;
        }
        Some(
            self.entries
                .iter()
                .map(|entry| entry.text.clone())
                .collect(),
        )
    }

    fn show_boolean(
        &mut self,
        ui: &mut Ui,
        field: &DraftField,
        position: usize,
        width: f32,
    ) -> egui::Response {
        let entry = &self.entries[position];
        let selected = if entry.choice == Choice::Value {
            if entry.text.eq_ignore_ascii_case("True") {
                Some(true)
            } else if entry.text.eq_ignore_ascii_case("False") {
                Some(false)
            } else {
                None
            }
        } else {
            None
        };
        let mut checked = selected.unwrap_or(false);
        let hint = tr(ui, entry.choice.key());
        let label = message(
            ui,
            "table-draft-record-field",
            &[("field", field.name().into())],
        );
        let (segment, _) = ui.allocate_exact_size(
            egui::vec2(width, ui.available_height()),
            egui::Sense::hover(),
        );
        // The mode selector owns the right gutter. Center the checkbox on the
        // complete field, while retaining that separate selector's hit area.
        let center = segment.center() + egui::vec2(INPUT_MODE_WIDTH / 2.0, 0.0);
        let bounds = egui::Rect::from_center_size(
            center,
            egui::vec2(ui.spacing().icon_width, segment.height()),
        );
        let response = ui.place(
            bounds,
            egui::Checkbox::without_text(&mut checked).indeterminate(selected.is_none()),
        );
        response.widget_info(|| {
            if let Some(selected) = selected {
                WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), selected, &label)
            } else {
                WidgetInfo::labeled(WidgetType::Checkbox, ui.is_enabled(), &label)
            }
        });
        if response.changed() {
            self.set_input(
                field.id(),
                InsertInput::Value(EditInput {
                    text: if checked { "True" } else { "False" }.into(),
                    is_null: false,
                }),
            );
        }
        response.on_hover_text(hint)
    }

    /// Render one cell; typing is explicit Value intent, not type inference.
    pub(super) fn show_cell(&mut self, ui: &mut Ui, field: &DraftField) -> bool {
        if field.kind() == DraftFieldType::AutoNumberIncrement {
            ui.label(tr(ui, "table-draft-new-record"))
                .on_hover_text(tr(ui, "insertion-auto-number-help"));
            return false;
        }
        let Some(position) = self.entries.iter().position(|entry| entry.id == field.id()) else {
            return false;
        };
        let entry = &self.entries[position];
        let original_revision = self.revision;
        let mut choice = entry.choice;
        let mut text = if choice == Choice::Value {
            entry.text.clone()
        } else {
            String::new()
        };
        let other_bytes = self
            .entries
            .iter()
            .filter(|entry| entry.id != field.id())
            .map(|entry| entry.text.len())
            .sum::<usize>();
        ui.push_id(("initial_record", field.id()), |ui| {
            ui.horizontal(|ui| {
                let width = (ui.available_width() - INPUT_MODE_WIDTH).max(MIN_INPUT_WIDTH);
                let response = if field.kind() == DraftFieldType::YesNo {
                    self.show_boolean(ui, field, position, width)
                } else {
                    let id = ui.make_persistent_id("value");
                    let remaining = MAX_RECORD_TEXT_BYTES.saturating_sub(other_bytes);
                    let rejected =
                        crate::text_input::reject_oversized_events(ui, id, &text, remaining);
                    if rejected {
                        self.reject(position);
                    }
                    let hint = tr(ui, choice.key());
                    let response = ui.add_sized(
                        [width, ui.available_height()],
                        egui::TextEdit::singleline(&mut text).id(id).hint_text(hint),
                    );
                    let label = message(
                        ui,
                        "table-draft-record-field",
                        &[("field", field.name().into())],
                    );
                    response
                        .widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, &label));
                    if response.changed() && !rejected {
                        choice = Choice::Value;
                        self.set_input(
                            field.id(),
                            InsertInput::Value(EditInput {
                                text: text.clone(),
                                is_null: false,
                            }),
                        );
                    }
                    response
                };
                let (menu, _) = egui::containers::menu::MenuButton::from_button(
                    egui::Button::new("")
                        .min_size(egui::vec2(INPUT_MODE_WIDTH, ui.available_height())),
                )
                .ui(ui, |ui| {
                    for mode in [Choice::Omitted, Choice::Null, Choice::Value] {
                        ui.add_enabled_ui(
                            mode != Choice::Null || field.kind() != DraftFieldType::YesNo,
                            |ui| {
                                if ui
                                    .radio_value(&mut choice, mode, tr(ui, mode.key()))
                                    .changed()
                                {
                                    let input = match choice {
                                        Choice::Omitted => InsertInput::Omitted,
                                        Choice::Null | Choice::Value => {
                                            InsertInput::Value(EditInput {
                                                text: self.entries[position].text.clone(),
                                                is_null: choice == Choice::Null,
                                            })
                                        }
                                    };
                                    self.set_input(field.id(), input);
                                    ui.close();
                                }
                            },
                        );
                    }
                });
                let center = menu.rect.center();
                let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.inactive.fg_stroke.color);
                ui.painter().line_segment(
                    [
                        center + egui::vec2(-MODE_CHEVRON_HALF_WIDTH, -MODE_CHEVRON_HALF_HEIGHT),
                        center + egui::vec2(0.0, MODE_CHEVRON_HALF_HEIGHT),
                    ],
                    stroke,
                );
                ui.painter().line_segment(
                    [
                        center + egui::vec2(0.0, MODE_CHEVRON_HALF_HEIGHT),
                        center + egui::vec2(MODE_CHEVRON_HALF_WIDTH, -MODE_CHEVRON_HALF_HEIGHT),
                    ],
                    stroke,
                );
                let mode_label = message(
                    ui,
                    "insertion-field-input",
                    &[("field", field.name().into())],
                );
                menu.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &mode_label));
                if self.entries[position].rejected {
                    response.on_hover_text(tr(ui, "insertion-input-limit"));
                }
            });
        });
        self.revision != original_revision
    }
}

impl Choice {
    fn key(self) -> &'static str {
        match self {
            Self::Omitted => "insertion-omitted",
            Self::Null => "insertion-null",
            Self::Value => "insertion-value",
        }
    }
}

#[cfg(test)]
mod tests;
