//! Modeless Find follows Microsoft's public dialog layout; matching is host-owned.
use super::replace::MAX_REPLACEMENT_BYTES;
use super::{FindDirection, FindMatch, MAX_FIND_PATTERN_BYTES, Session};
use crate::localization::{message, tr};
use crate::workspace::{Action, State, diagnostics};
use egui::{ComboBox, Id, Ui};

const MAX_EDITOR_SCALARS: usize = crate::text_input::MAX_EDITOR_SCALARS;
const FIND_WINDOW_WIDTH: f32 = 560.0;
const FIND_BUTTON_WIDTH: f32 = 110.0;
const FIND_VERTICAL_RESERVE: f32 = 160.0;
const FIND_BODY_HEIGHT: f32 = 260.0;

impl State {
    pub(in crate::workspace) fn find_window(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        if self.edit_dialog_open()
            || self.zoom_dialog_open()
            || self.contents_request_id().is_some()
            || self.saved_query_prompt_open()
        {
            if let Some(request_id) = self.close_find() {
                actions.push(Action::CancelFind { request_id });
            }
            return;
        }
        let Some((target, position)) = self.find_source() else {
            if let Some(request_id) = self.close_find() {
                actions.push(Action::CancelFind { request_id });
            }
            return;
        };
        let current_request = self.current_find_request();
        let replace_ready = self
            .current_replace_find_request()
            .as_ref()
            .is_some_and(|request| self.can_replace_find(request));
        let found_current = self.found_receipt_current();
        let stale_replace = self
            .replace_request_id()
            .filter(|id| !self.accepts_replace_find_reply(*id));
        let Some(session) = &mut self.find else {
            return;
        };
        if session.target != target || session.publication != self.focus_publication {
            if let Some(request_id) = self.close_find() {
                actions.push(Action::CancelFind { request_id });
            }
            return;
        }
        let stale_pending = session
            .pending
            .as_ref()
            .is_some_and(|pending| current_request.as_ref() != Some(&pending.request));
        if stale_pending && let Some(pending) = session.pending.take() {
            actions.push(Action::CancelFind {
                request_id: pending.request_id,
            });
        }
        if let Some(request_id) = stale_replace {
            session.replacement.pending = None;
            actions.push(Action::CancelFind { request_id });
        }
        if session.found.is_some() && !found_current {
            session.found = None;
            session.found_request = None;
        }
        if session.field != position.column {
            session.field = position.column;
            session.found = None;
            session.found_request = None;
            session.not_found = false;
            session.error = None;
        }

        let enabled =
            !self.busy && session.pending.is_none() && session.replacement.pending.is_none();
        let ready = self.find_available || self.mock_preview;
        let formatted_available = self.find_formatted_available || self.mock_preview;
        let popup_open = egui::Popup::is_any_open(context);
        let mut open = true;
        let mut close = false;
        let mut submit = false;
        let mut replace = false;
        egui::Window::new(self.localizer.text("find-title"))
            .id(Id::new("native_find_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(FIND_WINDOW_WIDTH)
            .default_pos(
                context.content_rect().center()
                    - egui::vec2(FIND_WINDOW_WIDTH, FIND_BODY_HEIGHT) * 0.5,
            )
            .constrain_to(context.content_rect())
            .show(context, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal(|ui| {
                        find_tab(ui, false, "find-tab", &mut session.replacement.visible);
                        find_tab(
                            ui,
                            true,
                            "find-replace-tab",
                            &mut session.replacement.visible,
                        );
                    });
                });
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                let height = (context.content_rect().height() - FIND_VERTICAL_RESERVE)
                    .clamp(120.0, FIND_BODY_HEIGHT);
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let controls_width = (ui.available_width()
                                - FIND_BUTTON_WIDTH
                                - ui.spacing().item_spacing.x)
                                .max(160.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(controls_width, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.add_enabled_ui(enabled, |ui| {
                                        submit |= controls(ui, session, formatted_available);
                                    });
                                },
                            );
                            ui.vertical(|ui| {
                                let search = ui.add_enabled(
                                    enabled
                                        && ready
                                        && !session.pattern.is_empty()
                                        && session.pattern.len() <= MAX_FIND_PATTERN_BYTES,
                                    egui::Button::new(tr(ui, "find-next"))
                                        .min_size(egui::vec2(FIND_BUTTON_WIDTH, 0.0)),
                                );
                                submit |= search.clicked();
                                if session.replacement.visible {
                                    replace |= ui
                                        .add_enabled(
                                            enabled && replace_ready,
                                            egui::Button::new(tr(ui, "find-replace-tab"))
                                                .min_size(egui::vec2(FIND_BUTTON_WIDTH, 0.0)),
                                        )
                                        .clicked();
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new(tr(ui, "find-replace-all"))
                                            .min_size(egui::vec2(FIND_BUTTON_WIDTH, 0.0)),
                                    )
                                    .on_disabled_hover_text(tr(ui, "find-replace-all-unavailable"));
                                }
                                close = ui
                                    .add_sized(
                                        egui::vec2(FIND_BUTTON_WIDTH, ui.spacing().interact_size.y),
                                        egui::Button::new(tr(ui, "shell-cancel")),
                                    )
                                    .clicked();
                            });
                        });
                        if session.pattern.len() > MAX_FIND_PATTERN_BYTES {
                            ui.label(tr(ui, "find-pattern-limit"));
                        }
                        if session.replacement.visible {
                            ui.small(tr(ui, "find-replace-review-help"));
                            if !self.replace_available && !self.mock_preview {
                                ui.small(tr(ui, "find-replace-unavailable"));
                            }
                        }
                        if session.pending.is_some() || session.replacement.pending.is_some() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr(
                                    ui,
                                    if session.replacement.pending.is_some() {
                                        "find-replacing"
                                    } else {
                                        "find-searching"
                                    },
                                ));
                            });
                        }
                        if session.not_found {
                            ui.label(tr(ui, "find-not-found"));
                        }
                        if let Some(found) = &session.found {
                            ui.label(message(
                                ui,
                                "find-found",
                                &[
                                    ("row", (u64::from(found.parent_ordinal) + 1).to_string()),
                                    ("column", found.column.clone()),
                                ],
                            ));
                        }
                        if let Some(error) = &session.error {
                            diagnostics::show(ui, ui.id().with("find_error"), error);
                        }
                        if self.mock_preview {
                            ui.small(tr(
                                ui,
                                if session.replacement.visible {
                                    "find-replace-mock-help"
                                } else {
                                    "find-mock-help"
                                },
                            ));
                        }
                        if !ready {
                            ui.small(tr(ui, "find-unavailable"));
                        }
                    });
            });
        let escape = !popup_open
            && !egui::Popup::is_any_open(context)
            && context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        close |= !open || escape;
        if close {
            if let Some(request_id) = self.close_find() {
                actions.push(Action::CancelFind { request_id });
            }
        } else if replace
            && enabled
            && let Some(request) = self.current_replace_find_request()
            && self.can_replace_find(&request)
        {
            actions.push(Action::ReplaceFindMatch { request });
        } else if submit
            && enabled
            && ready
            && let Some(request) = self.current_find_request()
            && self.can_find_next(&request)
        {
            actions.push(Action::FindNext { request });
        }
    }
}

fn controls(ui: &mut Ui, session: &mut Session, formatted_available: bool) -> bool {
    let mut submit = false;
    let mut changed = false;
    let mut rejected = None;
    egui::Grid::new("find_controls")
        .num_columns(2)
        .show(ui, |ui| {
            let label = ui.label(tr(ui, "find-what"));
            let id = ui.make_persistent_id("find_pattern");
            let rejected_pattern = crate::text_input::reject_oversized_events(
                ui,
                id,
                &session.pattern,
                MAX_FIND_PATTERN_BYTES,
            );
            let input = ui
                .add(
                    egui::TextEdit::singleline(&mut session.pattern)
                        .id(id)
                        // Bound editor layout by scalars. The independent UTF-8
                        // request byte limit still rejects large non-ASCII input.
                        .char_limit(MAX_EDITOR_SCALARS)
                        .desired_width(ui.available_width()),
                )
                .labelled_by(label.id);
            if session.focus_input {
                input.request_focus();
                session.focus_input = false;
            }
            changed |= input.changed();
            if rejected_pattern {
                rejected = Some("find-pattern-limit");
            }
            submit |= (input.has_focus() || input.lost_focus())
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            ui.end_row();

            if session.replacement.visible {
                let label = ui.label(tr(ui, "find-replace-with"));
                let id = ui.make_persistent_id("find_replacement");
                let rejected_replacement = crate::text_input::reject_oversized_events(
                    ui,
                    id,
                    &session.replacement.text,
                    MAX_REPLACEMENT_BYTES,
                );
                ui.add(
                    egui::TextEdit::singleline(&mut session.replacement.text)
                        .id(id)
                        .char_limit(MAX_EDITOR_SCALARS)
                        .desired_width(ui.available_width()),
                )
                .labelled_by(label.id);
                if rejected_replacement {
                    rejected = Some("find-replacement-limit");
                }
                ui.end_row();
            }

            let label = ui.label(tr(ui, "find-look-in"));
            ComboBox::from_id_salt("find_scope")
                .selected_text(if session.whole_table {
                    &session.target.table
                } else {
                    &session.field
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut session.whole_table, false, &session.field)
                        .changed();
                    changed |= ui
                        .selectable_value(&mut session.whole_table, true, &session.target.table)
                        .changed();
                })
                .response
                .labelled_by(label.id);
            ui.end_row();

            let label = ui.label(tr(ui, "find-match"));
            ComboBox::from_id_salt("find_match")
                .selected_text(tr(ui, match_key(session.matching)))
                .show_ui(ui, |ui| {
                    for value in [FindMatch::Any, FindMatch::Whole, FindMatch::Start] {
                        changed |= ui
                            .selectable_value(
                                &mut session.matching,
                                value,
                                tr(ui, match_key(value)),
                            )
                            .changed();
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();

            let label = ui.label(tr(ui, "find-search"));
            ComboBox::from_id_salt("find_direction")
                .selected_text(tr(ui, direction_key(session.direction)))
                .show_ui(ui, |ui| {
                    for value in [FindDirection::All, FindDirection::Up, FindDirection::Down] {
                        changed |= ui
                            .selectable_value(
                                &mut session.direction,
                                value,
                                tr(ui, direction_key(value)),
                            )
                            .changed();
                    }
                })
                .response
                .labelled_by(label.id);
            ui.end_row();
        });
    changed |= ui
        .checkbox(&mut session.match_case, tr(ui, "find-match-case"))
        .changed();
    changed |= ui
        .add_enabled(
            formatted_available,
            egui::Checkbox::new(&mut session.formatted, tr(ui, "find-formatted")),
        )
        .on_disabled_hover_text(tr(ui, "find-formatted-unavailable"))
        .changed();
    if changed {
        session.found = None;
        session.found_request = None;
        session.not_found = false;
        session.error = None;
    }
    if let Some(key) = rejected {
        session.error = Some(tr(ui, key));
    }
    submit
}

fn match_key(value: FindMatch) -> &'static str {
    match value {
        FindMatch::Any => "find-any",
        FindMatch::Whole => "find-whole",
        FindMatch::Start => "find-start",
    }
}

fn direction_key(value: FindDirection) -> &'static str {
    match value {
        FindDirection::All => "find-all",
        FindDirection::Up => "find-up",
        FindDirection::Down => "find-down",
    }
}

fn find_tab(ui: &mut Ui, replacement: bool, key: &str, selected: &mut bool) {
    let active = *selected == replacement;
    let response = ui.selectable_label(active, tr(ui, key));
    if response.clicked() {
        *selected = replacement;
    }
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Tab);
        node.clear_toggled();
        node.set_selected(active);
    });
}
