//! Native type metadata determines the available common-filter controls.
use super::{FieldFilterPatternMode, MAX_INPUT_BYTES};
use crate::text_input;
use crate::localization::tr;
use crate::workspace::{Action, State, diagnostics};
use egui::{ComboBox, Id, Label, Popup, TextEdit, TextStyle, Ui};

const MODAL_MARGIN: i8 = 12;
const MODAL_VERTICAL_RESERVE: f32 = 180.0;
const MIN_BODY_HEIGHT: f32 = 100.0;

impl State {
    pub(in crate::workspace) fn field_filter_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(current) = &self.table_filter.field else {
            return;
        };
        let source_current = self.field_witness_current(current);
        let busy = self.busy;
        let popup_was_open = Popup::is_any_open(context);
        let localizer = self.localizer.clone();
        let field = self
            .table_filter
            .field
            .as_mut()
            .expect("existing field dialog");
        let mut close = false;
        let frame =
            egui::Frame::popup(&context.style_of(context.theme())).inner_margin(MODAL_MARGIN);
        egui::Modal::new(Id::new((
            "field_filter",
            field.object.id,
            &field.target.column,
        )))
        .frame(frame)
        .show(context, |ui| {
            ui.set_width((context.content_rect().width() - 80.0).clamp(280.0, 640.0));
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            ui.heading(tr(ui, "filtering-field-title"));
            let body_height =
                (context.content_rect().height() - MODAL_VERTICAL_RESERVE).max(MIN_BODY_HEIGHT);
            egui::ScrollArea::vertical()
                .id_salt("field_filter_body")
                .auto_shrink([false, true])
                .max_height(body_height)
                .show(ui, |ui| {
                    for name in [&field.target.table, &field.target.column] {
                        ui.add(Label::new(name).truncate()).on_hover_text(name);
                    }
                    ui.separator();
                    if let Some(description) = &field.description {
                        match description.pattern_mode {
                            Some(FieldFilterPatternMode::Ansi89) => {
                                ui.label(tr(ui, "filtering-field-ansi89"));
                            }
                            Some(FieldFilterPatternMode::Ansi92) => {
                                ui.label(tr(ui, "filtering-field-ansi92"));
                            }
                            None => {
                                ui.label(description.input_hint.label(&localizer));
                            }
                        }
                        ui.add_enabled_ui(!busy && !field.applying, |ui| {
                            let label = tr(ui, "filtering-field-comparison");
                            ui.label(&label);
                            let response = ComboBox::from_id_salt("field_filter_operator")
                                .selected_text(localizer.text(field.operator.key()))
                                .show_ui(ui, |ui| {
                                    for operator in &description.operators {
                                        ui.selectable_value(
                                            &mut field.operator,
                                            *operator,
                                            localizer.text(operator.key()),
                                        );
                                    }
                                })
                                .response;
                            ui.ctx()
                                .accesskit_node_builder(response.id, |node| node.set_label(label));
                            for index in 0..field.operator.operand_count() {
                                let label = tr(
                                    ui,
                                    if index == 0 {
                                        "filtering-field-value"
                                    } else {
                                        "filtering-field-second-value"
                                    },
                                );
                                ui.label(&label);
                                let id = ui.make_persistent_id(("field_operand", index));
                                if text_input::reject_oversized_events(
                                    ui,
                                    id,
                                    &field.inputs[index],
                                    MAX_INPUT_BYTES,
                                ) {
                                    field.error = Some(localizer.text("filtering-field-limit"));
                                }
                                let response = ui.add(
                                    TextEdit::multiline(&mut field.inputs[index])
                                        .id(id)
                                        .char_limit(text_input::MAX_EDITOR_SCALARS)
                                        .desired_rows(2)
                                        .desired_width(ui.available_width()),
                                );
                                crate::theme::input_border(ui, &response);
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_label(label)
                                });
                            }
                        });
                        if !field.inputs_fit() {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                tr(ui, "filtering-field-limit"),
                            );
                        }
                        if field.applying {
                            ui.spinner();
                            ui.label(tr(ui, "filtering-field-applying"));
                        }
                    } else if field.error.is_none() {
                        ui.spinner();
                        ui.label(tr(ui, "filtering-field-checking"));
                    }
                    if let Some(error) = &field.error {
                        diagnostics::show(ui, ui.id().with("field_filter_error"), error);
                    }
                });
            ui.separator();
            let apply = tr(ui, "filtering-field-apply");
            let cancel = tr(ui, "shell-cancel");
            let footer_width =
                button_width(ui, &apply) + button_width(ui, &cancel) + ui.spacing().item_spacing.x;
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - footer_width).max(0.0));
                let apply_enabled = source_current
                    && !busy
                    && !field.applying
                    && field.description.is_some()
                    && field.inputs_fit();
                let response = ui
                    .push_id("field_filter_apply", |ui| {
                        ui.add_enabled(apply_enabled, egui::Button::new(apply))
                    })
                    .inner;
                if response.clicked() {
                    actions.push(Action::ApplyFieldFilter {
                        target: field.target.clone(),
                        operator: field.operator,
                        inputs: field.inputs(),
                    });
                }
                close |= ui
                    .push_id("field_filter_cancel", |ui| ui.button(cancel))
                    .inner
                    .clicked();
            });
        });
        if !popup_was_open
            && !Popup::is_any_open(context)
            && context
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close = true;
        }
        if close {
            actions.push(Action::CloseFieldFilter {
                request_id: field.request_id,
            });
        }
    }
}

fn button_width(ui: &Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            TextStyle::Button.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
        + ui.spacing().button_padding.x * 2.0
}
