//! Startup content has no document-return action or fabricated recent history.
use super::{Backstage, State};
use crate::localization::tr;
use crate::theme::palette;
use crate::workspace::{Action, ribbon::Icon};
use egui::{Align, Color32, Frame, Layout, Margin, Response, RichText, ScrollArea, Ui};

const STARTUP_RAIL_WIDTH: f32 = 250.0;
const STARTUP_RAIL_FILL: Color32 = Color32::from_rgb(164, 55, 58);
const TEMPLATE_WIDTH: f32 = 210.0;
const TEMPLATE_HEIGHT: f32 = 200.0;
const TEMPLATE_MARGIN: i8 = 12;
const TEMPLATE_ICON_SIZE: f32 = 80.0;

impl State {
    pub(super) fn startup(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        egui::Panel::left("startup_recent")
            .exact_size(STARTUP_RAIL_WIDTH)
            .frame(
                Frame::new()
                    .fill(STARTUP_RAIL_FILL)
                    .inner_margin(Margin::same(20)),
            )
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::WHITE);
                ui.label(RichText::new("TableCraft").size(30.0));
                ui.add_space(24.0);
                ui.label(RichText::new(tr(ui, "backstage-recent")).size(22.0));
                ui.label(tr(ui, "backstage-recent-empty"));
                ui.add_space(18.0);
                ui.separator();
                if startup_entry(ui, &tr(ui, "backstage-open-other")).clicked() {
                    self.backstage = Backstage::Open;
                }
                let footer_height = 2.0 * super::rail::ROW_HEIGHT;
                ui.add_space((ui.available_height() - footer_height).max(0.0));
                if startup_entry(ui, &tr(ui, "backstage-about")).clicked() {
                    self.backstage = Backstage::About;
                }
                if startup_entry(ui, &tr(ui, "backstage-options")).clicked() {
                    self.backstage = Backstage::Options;
                }
            });
        egui::CentralPanel::default()
            .frame(Frame::central_panel(ui.style()).fill(palette(ui.ctx()).title_fill))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("startup_templates")
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        ui.heading(tr(ui, "backstage-home"));
                        ui.add_space(18.0);
                        if self.file_picker_available
                            && ui
                                .add_enabled(
                                    self.file_picker_ready(),
                                    egui::Button::new(tr(ui, "backstage-browse")),
                                )
                                .clicked()
                        {
                            actions.push(Action::ChooseFile);
                        }
                        if self.mock_preview {
                            ui.add_space(18.0);
                            let mut search = String::new();
                            ui.add_enabled(
                                false,
                                egui::TextEdit::singleline(&mut search)
                                    .desired_width(280.0)
                                    .hint_text(tr(ui, "backstage-template-search")),
                            )
                            .on_disabled_hover_text(tr(ui, "backstage-creation-unavailable"));
                            ui.add_space(18.0);
                            ui.heading(tr(ui, "backstage-templates"));
                            self.blank_template(ui);
                        } else {
                            ui.label(tr(ui, "backstage-open-title"));
                        }
                    });
            });
    }

    pub(super) fn start_pane(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        ui.heading(tr(ui, "backstage-home"));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if ui.button(tr(ui, "backstage-open")).clicked() {
                self.backstage = Backstage::Open;
            }
            if self.file_picker_available
                && ui
                    .add_enabled(
                        self.file_picker_ready(),
                        egui::Button::new(tr(ui, "backstage-browse")),
                    )
                    .clicked()
            {
                actions.push(Action::ChooseFile);
            }
        });
        ui.add_space(24.0);
        ui.heading(tr(ui, "backstage-recent"));
        ui.label(tr(ui, "backstage-recent-empty"));
        if self.mock_preview {
            ui.add_space(24.0);
            ui.heading(tr(ui, "backstage-new"));
            self.blank_template(ui);
        }
    }

    pub(super) fn new_pane(&self, ui: &mut Ui) {
        ui.heading(tr(ui, "backstage-new"));
        ui.add_space(18.0);
        ui.label(tr(ui, "backstage-creation-unavailable"));
        self.blank_template(ui);
    }

    fn blank_template(&self, ui: &mut Ui) {
        let colors = palette(ui.ctx());
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            egui::vec2(TEMPLATE_WIDTH, TEMPLATE_HEIGHT),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_max_width(TEMPLATE_WIDTH);
                ui.add_enabled_ui(false, |ui| {
                    // Neutral artwork already signals unavailability. Avoid an
                    // additional opacity fade that makes its caption unreadable.
                    ui.set_opacity(1.0);
                    Frame::new()
                        .fill(colors.surface)
                        .stroke(egui::Stroke::new(1.0, colors.grid_line))
                        .inner_margin(Margin::same(TEMPLATE_MARGIN))
                        .show(ui, |ui| {
                            let inset = 2.0 * f32::from(TEMPLATE_MARGIN);
                            ui.set_min_size(egui::vec2(
                                TEMPLATE_WIDTH - inset,
                                TEMPLATE_HEIGHT - inset,
                            ));
                            ui.vertical_centered(|ui| {
                                ui.add(Icon::DatabaseFile.image(ui, TEMPLATE_ICON_SIZE));
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(tr(ui, "backstage-blank-database")),
                                )
                                .on_disabled_hover_text(tr(ui, "backstage-creation-unavailable"));
                                ui.small(tr(ui, "backstage-creation-unavailable"));
                            });
                        });
                });
            },
        );
    }
}

fn startup_entry(ui: &mut Ui, label: &str) -> Response {
    let response = ui
        .scope(|ui| {
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            widgets.hovered.weak_bg_fill = Color32::from_white_alpha(30);
            widgets.active.weak_bg_fill = Color32::from_white_alpha(45);
            ui.add(
                egui::Button::new("")
                    .left_text(RichText::new(label).color(Color32::WHITE))
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(0.0)
                    .min_size(egui::vec2(ui.available_width(), super::rail::ROW_HEIGHT)),
            )
        })
        .inner;
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
    });
    response
}
