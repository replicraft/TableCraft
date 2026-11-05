//! Shared title, contextual ribbon and status; no file or database services.
use super::focus::{self, Pane};
use super::{Action, DocumentView, State, ribbon};
use crate::theme::palette;
use egui::{Frame, Margin, RichText, Ui};

impl State {
    pub(super) fn chrome(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let colors = palette(ui.ctx());
        let t = self.localizer.clone();
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(colors.title_fill)
                    .inner_margin(Margin::symmetric(10, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t.text("shell-app-name"))
                            .color(colors.accent)
                            .strong()
                            .size(13.0),
                    );
                    ui.separator();
                    let title = self
                        .database
                        .as_ref()
                        .map_or_else(|| t.text("shell-workspace"), |db| db.title.clone());
                    let status = RichText::new(self.edit_status())
                        .color(colors.muted)
                        .size(11.0);
                    let status_width = egui::WidgetText::from(status.clone())
                        .into_galley(
                            ui,
                            Some(egui::TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Body,
                        )
                        .size()
                        .x;
                    let title_width =
                        (ui.available_width() - status_width - ui.spacing().item_spacing.x)
                            .max(0.0);
                    let response = ui
                        .allocate_ui_with_layout(
                            egui::vec2(title_width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown)
                                .with_cross_align(egui::Align::LEFT),
                            |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(&title).color(colors.ink))
                                        .halign(egui::Align::LEFT)
                                        .truncate(),
                                )
                            },
                        )
                        .inner
                        .on_hover_text(&title);
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_label(title);
                    });
                    ui.label(status);
                });
            });
        if !self.backstage_visible() {
            egui::Panel::top("ribbon")
                .frame(
                    Frame::new()
                        .fill(colors.surface)
                        .inner_margin(Margin::symmetric(8, 3)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("ribbon_tabs")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let file =
                                    ui.add(egui::Button::new(t.text("ribbon-file")).frame(false));
                                focus::register(ui, Pane::Ribbon, &file, false);
                                if file.clicked() {
                                    self.show_open();
                                }
                                if tab(
                                    ui,
                                    "ribbon-home",
                                    &t.text("ribbon-home"),
                                    self.ribbon_tab == ribbon::Tab::Home,
                                    true,
                                )
                                .clicked()
                                {
                                    self.ribbon_tab = ribbon::Tab::Home;
                                }
                                if self.mock_preview {
                                    for key in [
                                        "ribbon-create",
                                        "ribbon-external-data",
                                        "ribbon-database-tools",
                                        "ribbon-help",
                                    ] {
                                        tab(ui, key, &t.text(key), false, false)
                                            .on_disabled_hover_text(t.text("ribbon-preview-only"));
                                    }
                                }
                                if self.view == DocumentView::Design {
                                    if tab(
                                        ui,
                                        "ribbon-table-design",
                                        &t.text("ribbon-table-design"),
                                        self.ribbon_tab == ribbon::Tab::TableDesign,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        self.ribbon_tab = ribbon::Tab::TableDesign;
                                    }
                                } else if self.mock_preview && self.data.is_some() {
                                    for key in ["ribbon-table-fields", "ribbon-table"] {
                                        tab(ui, key, &t.text(key), false, false)
                                            .on_disabled_hover_text(t.text("ribbon-preview-only"));
                                    }
                                }
                            });
                        });
                    ui.separator();
                    self.home(ui, actions);
                });
        }
        egui::Panel::bottom("status")
            .frame(
                Frame::new()
                    .fill(colors.title_fill)
                    .inner_margin(Margin::symmetric(8, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.small(t.text(if self.busy {
                        "shell-working"
                    } else {
                        "shell-ready"
                    }));
                    if self.busy {
                        ui.spinner();
                        if ui.button(t.text("shell-cancel")).clicked() {
                            actions.push(Action::Cancel);
                        }
                    }
                    ui.separator();
                    ui.small(
                        self.database.as_ref().map_or_else(
                            || t.text("shell-local-databases"),
                            |db| db.format.clone(),
                        ),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(t.text("shell-snapshot-browsing"));
                    });
                });
            });
    }
}

fn tab(ui: &mut Ui, key: &str, label: &str, selected: bool, enabled: bool) -> egui::Response {
    let colors = palette(ui.ctx());
    let response = ui
        .push_id(key, |ui| {
            ui.add_enabled(
                enabled,
                egui::Button::selectable(selected, label)
                    .fill(colors.surface)
                    .frame(false),
            )
        })
        .inner;
    focus::register(ui, Pane::Ribbon, &response, selected);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Tab);
        node.clear_toggled();
        node.set_selected(selected);
        if !response.enabled() {
            node.remove_action(egui::accesskit::Action::Click);
            node.remove_action(egui::accesskit::Action::Focus);
        }
    });
    if response.enabled() && response.gained_focus() {
        response.scroll_to_me(None);
    }
    if selected {
        ui.painter().line_segment(
            [response.rect.left_bottom(), response.rect.right_bottom()],
            egui::Stroke::new(2.0, colors.accent),
        );
    }
    if response.enabled() && response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            egui::Stroke::new(1.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

#[cfg(test)]
mod tests;
