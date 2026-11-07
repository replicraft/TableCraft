//! One source-bound draft row; no stored ordinal or generated value is invented.
use super::{
    Choice, Dialog, FieldInput, InsertDraft, entries_within_limits, generated_entries_omitted,
    inputs,
};
use crate::localization::{message, tr};
use crate::workspace::{Action, State};
use egui::{Id, Key, Modifiers, Ui};

const INPUT_MODE_WIDTH: f32 = 20.0;
const MIN_INPUT_WIDTH: f32 = 24.0;
const STALE_DETAILS_HEIGHT: f32 = 140.0;
const MODE_CHEVRON_HALF_WIDTH: f32 = 3.0;
const MODE_CHEVRON_HALF_HEIGHT: f32 = 1.5;

#[derive(Default)]
pub(in crate::workspace) struct Editor {
    checkpoint: Option<(usize, FieldInput)>,
    focus: Option<usize>,
    entered: bool,
    last_focus: Option<(u64, usize)>,
}

impl Editor {
    pub(super) fn refocus(&mut self, position: usize) {
        self.focus = Some(position);
        self.entered = false;
    }

    pub(super) fn restore_focus(&mut self) {
        self.focus = self.checkpoint.as_ref().map(|(position, _)| *position);
        self.entered = false;
    }
}

pub(crate) struct Row<'a> {
    draft: &'a InsertDraft,
    entries: &'a mut [FieldInput],
    editor: &'a mut Editor,
    request_id: u64,
}

pub(in crate::workspace) fn row<'a>(
    dialog: &'a mut Option<Dialog>,
    data: &crate::Data,
) -> Option<Row<'a>> {
    let Dialog::Draft {
        draft,
        entries,
        inline,
        request_id,
        pending: None,
        review: None,
        ..
    } = dialog.as_mut()?
    else {
        return None;
    };
    if data.source_revision != Some(draft.target.document_revision)
        || data.table != draft.target.table
        || !super::draft_columns_match(draft, data)
    {
        return None;
    }
    Some(Row {
        draft,
        entries,
        editor: inline,
        request_id: *request_id,
    })
}

impl Row<'_> {
    pub(crate) fn header_active(&self, column: &str) -> bool {
        let position = self
            .editor
            .focus
            .or_else(|| {
                self.editor
                    .checkpoint
                    .as_ref()
                    .map(|(position, _)| *position)
            })
            .or_else(|| {
                if self.editor.entered {
                    None
                } else {
                    self.draft
                        .fields
                        .iter()
                        .position(|field| !field.auto_number)
                }
            });
        position
            .and_then(|position| self.draft.fields.get(position))
            .is_some_and(|field| field.name == column && !field.auto_number)
    }

    pub(crate) fn needs_reveal(&self) -> bool {
        !self.editor.entered
    }

    pub(crate) fn cell(&mut self, ui: &mut Ui, column: &str, actions: &mut Vec<Action>) {
        let Some(position) = self
            .draft
            .fields
            .iter()
            .position(|field| field.name == column)
        else {
            ui.label("—").on_hover_text(tr(ui, "ribbon-unavailable"));
            return;
        };
        let field = &self.draft.fields[position];
        if field.auto_number {
            ui.centered_and_justified(|ui| ui.label(tr(ui, "insertion-generated-value")));
            return;
        }
        if !self.editor.entered {
            self.editor.focus = self.editor.focus.or_else(|| {
                self.draft
                    .fields
                    .iter()
                    .position(|field| !field.auto_number)
            });
            self.editor.entered = true;
        }
        let remaining = super::remaining_input_bytes(self.entries, position);
        let entry = &mut self.entries[position];
        let id = Id::new(("inline_insert_field", self.draft.id, position));
        let focused = ui.memory(|memory| memory.has_focus(id));
        if focused
            && self
                .editor
                .checkpoint
                .as_ref()
                .is_none_or(|(field, _)| *field != position)
        {
            self.editor.checkpoint = Some((position, entry.clone()));
        }
        // egui clears focused_widget for Escape before the UI pass. The
        // previous rendered field is a comparison witness, never a stored
        // widget identifier to restore across source/layout changes.
        let frame = ui.ctx().cumulative_frame_nr();
        let escape_owner = focused
            || (ui.memory(|memory| memory.focused()).is_none()
                && !egui::Popup::is_any_open(ui.ctx())
                && self.editor.last_focus.is_some_and(|(previous, field)| {
                    field == position && frame.saturating_sub(previous) <= 1
                }));
        if escape_owner && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
            self.editor.focus = Some(position);
            if let Some((_, checkpoint)) = &self.editor.checkpoint
                && entry != checkpoint
            {
                *entry = checkpoint.clone();
            } else {
                actions.push(Action::CloseInsert {
                    request_id: self.request_id,
                    draft_id: Some(self.draft.id),
                });
            }
        }
        let review_requested =
            focused && ui.input_mut(|input| input.consume_key(Modifiers::SHIFT, Key::Enter));
        let tab = focused
            .then(|| {
                ui.input_mut(|input| {
                    if input.consume_key(Modifiers::SHIFT, Key::Tab) {
                        Some(false)
                    } else if input.consume_key(Modifiers::NONE, Key::Tab) {
                        Some(true)
                    } else {
                        None
                    }
                })
            })
            .flatten();
        if let Some(forward) = tab {
            let eligible: Vec<_> = self
                .draft
                .fields
                .iter()
                .enumerate()
                .filter_map(|(index, field)| (!field.auto_number).then_some(index))
                .collect();
            let current = eligible
                .iter()
                .position(|index| *index == position)
                .unwrap_or(0);
            let next = if forward {
                (current + 1) % eligible.len()
            } else {
                (current + eligible.len() - 1) % eligible.len()
            };
            self.editor.focus = Some(eligible[next]);
            self.editor.checkpoint = None;
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        let hint = tr(
            ui,
            match entry.choice {
                Choice::Omitted => "insertion-omitted",
                Choice::Null => "insertion-null",
                Choice::Value => "insertion-value",
            },
        );
        ui.horizontal(|ui| {
            let width = (ui.available_width() - INPUT_MODE_WIDTH).max(MIN_INPUT_WIDTH);
            let mut absent = String::new();
            let rendered = if entry.choice == Choice::Value {
                &mut entry.text
            } else {
                &mut absent
            };
            let rejected = crate::text_input::reject_oversized_events(ui, id, rendered, remaining);
            entry.input_rejected |= rejected;
            let response = ui.add_sized(
                [width, ui.available_height()],
                egui::TextEdit::singleline(rendered).id(id).hint_text(hint),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &field.name)
            });
            if self.editor.focus == Some(position) {
                response.request_focus();
                self.editor.focus = None;
                self.editor.checkpoint = Some((position, entry.clone()));
                response.scroll_to_me(None);
            }
            if response.has_focus() || ui.memory(|memory| memory.has_focus(id)) {
                self.editor.last_focus = Some((frame, position));
            }
            response.clone().on_hover_text(
                field
                    .input_hint
                    .label(&crate::localization::for_context(ui.ctx())),
            );
            if response.changed() {
                if !rejected {
                    entry.input_rejected = false;
                }
                if entry.choice != Choice::Value {
                    entry.text = absent;
                }
                entry.choice = Choice::Value;
            }
            let (menu, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("").min_size(egui::vec2(INPUT_MODE_WIDTH, ui.available_height())),
            )
            .ui(ui, |ui| {
                if ui
                    .radio_value(
                        &mut entry.choice,
                        Choice::Omitted,
                        tr(ui, "insertion-omitted"),
                    )
                    .changed()
                {
                    entry.input_rejected = false;
                    ui.close();
                }
                ui.add_enabled_ui(field.nullable, |ui| {
                    if ui
                        .radio_value(&mut entry.choice, Choice::Null, tr(ui, "insertion-null"))
                        .changed()
                    {
                        entry.input_rejected = false;
                        ui.close();
                    }
                });
                if ui
                    .radio_value(&mut entry.choice, Choice::Value, tr(ui, "insertion-value"))
                    .changed()
                {
                    entry.input_rejected = false;
                    self.editor.focus = Some(position);
                    ui.close();
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
            let label = message(
                ui,
                "insertion-field-input",
                &[("field", field.name.clone())],
            );
            menu.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        });
        if review_requested
            && entries_within_limits(self.entries)
            && generated_entries_omitted(&self.draft.fields, self.entries)
        {
            actions.push(Action::ReviewInsert {
                draft_id: self.draft.id,
                inputs: inputs(self.entries),
            });
        }
    }
}

impl State {
    pub(in crate::workspace) fn inline_insert_toolbar(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        let current = match &self.insertion {
            Some(Dialog::Draft { draft, .. }) => self.insert_draft_current(draft),
            _ => return,
        };
        let Some(Dialog::Draft {
            draft,
            entries,
            request_id,
            pending: None,
            review: None,
            error,
            ..
        }) = &mut self.insertion
        else {
            return;
        };
        ui.label(tr(ui, "insertion-inline-help"));
        if !current {
            ui.label(tr(ui, "insertion-stale-draft"));
        }
        if let Some(error) = error {
            ui.colored_label(ui.visuals().error_fg_color, error.as_str());
        }
        if !current {
            egui::ScrollArea::vertical()
                .max_height(STALE_DETAILS_HEIGHT)
                .show(ui, |ui| {
                    for (field, entry) in draft.fields.iter().zip(entries.iter()) {
                        ui.label(&field.name);
                        let mut retained = entry.text.as_str();
                        ui.add(egui::TextEdit::singleline(&mut retained).interactive(false));
                    }
                });
        }
        if !entries_within_limits(entries) {
            ui.colored_label(ui.visuals().error_fg_color, tr(ui, "insertion-input-limit"));
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    current
                        && !self.busy
                        && entries_within_limits(entries)
                        && generated_entries_omitted(&draft.fields, entries),
                    egui::Button::new(tr(ui, "insertion-review")),
                )
                .clicked()
            {
                actions.push(Action::ReviewInsert {
                    draft_id: draft.id,
                    inputs: inputs(entries),
                });
            }
            if ui.button(tr(ui, "shell-cancel")).clicked() {
                actions.push(Action::CloseInsert {
                    request_id: *request_id,
                    draft_id: Some(draft.id),
                });
            }
        });
    }
}
