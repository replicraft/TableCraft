//! Shared title, contextual ribbon and status; no file or database services.
use super::focus::{self, Pane};
use super::ribbon::Icon;
use super::{Action, DocumentView, State, ribbon};
use crate::Localizer;
use crate::theme::palette;
use egui::{Frame, Margin, RichText, Ui, WidgetInfo, WidgetType};

const APPLICATION_ICON_SIZE: f32 = 16.0;
const QUICK_ACCESS_BUTTON_SIZE: f32 = 20.0;
const RIBBON_TAB_HEIGHT: f32 = 26.0;
const RIBBON_TAB_SIDE_PADDING: f32 = 9.0;

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
                    ui.add(
                        Icon::Application
                            .image(ui, APPLICATION_ICON_SIZE)
                            .alt_text(t.text("shell-app-name")),
                    )
                    .on_hover_text(t.text("shell-app-name"));
                    if self.mock_preview {
                        quick_access(ui, &t);
                        ui.add_space(6.0);
                    }
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
            egui::Panel::top("ribbon_tabs")
                .frame(
                    Frame::new()
                        .fill(colors.title_fill)
                        .inner_margin(Margin::symmetric(8, 0)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("ribbon_tabs")
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.spacing_mut().button_padding = egui::vec2(9.0, 3.0);
                                let file_label = t.text("ribbon-file");
                                let file_size = ribbon_tab_size(ui, &file_label);
                                let file = ui.add(
                                    egui::Button::new(file_label)
                                        .frame(false)
                                        .min_size(file_size),
                                );
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
                                    false,
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
                                        tab(ui, key, &t.text(key), false, false, false)
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
                                        true,
                                    )
                                    .clicked()
                                    {
                                        self.ribbon_tab = ribbon::Tab::TableDesign;
                                    }
                                } else if self.mock_preview && self.data.is_some() {
                                    for key in ["ribbon-table-fields", "ribbon-table"] {
                                        tab(ui, key, &t.text(key), false, false, true)
                                            .on_disabled_hover_text(t.text("ribbon-preview-only"));
                                    }
                                }
                            });
                        });
                });
            egui::Panel::top("ribbon")
                .frame(Frame::new().fill(colors.title_fill).inner_margin(Margin {
                    left: 4,
                    right: 4,
                    top: 0,
                    bottom: 4,
                }))
                .show(ui, |ui| {
                    Frame::new()
                        .fill(colors.surface)
                        .corner_radius(5)
                        .stroke(egui::Stroke::new(1.0, colors.grid_line))
                        .inner_margin(Margin::symmetric(8, 3))
                        .show(ui, |ui| self.home(ui, actions));
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

fn quick_access(ui: &mut Ui, localizer: &Localizer) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
        ui.spacing_mut().interact_size = egui::Vec2::splat(QUICK_ACCESS_BUTTON_SIZE);
        for (key, icon) in [
            ("ribbon-save", Icon::Save),
            ("ribbon-undo", Icon::Undo),
            ("ribbon-redo", Icon::Redo),
        ] {
            let label = localizer.text(key);
            let response = ui
                .push_id(("quick_access", key), |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        ui.add(
                            egui::Button::image(icon.image(ui, APPLICATION_ICON_SIZE)).frame(false),
                        )
                    })
                    .inner
                })
                .inner;
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, false, &label));
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.remove_action(egui::accesskit::Action::Click);
                node.remove_action(egui::accesskit::Action::Focus);
            });
            response.on_disabled_hover_text(format!(
                "{label}: {}",
                localizer.text("ribbon-preview-only")
            ));
        }
    });
}

fn ribbon_tab_size(ui: &Ui, label: &str) -> egui::Vec2 {
    let font = ui.style().text_styles[&egui::TextStyle::Button].clone();
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, palette(ui.ctx()).ink);
    egui::vec2(
        text.size().x + RIBBON_TAB_SIDE_PADDING * 2.0,
        RIBBON_TAB_HEIGHT,
    )
}

fn tab(
    ui: &mut Ui,
    key: &str,
    label: &str,
    selected: bool,
    enabled: bool,
    contextual: bool,
) -> egui::Response {
    let colors = palette(ui.ctx());
    let size = ribbon_tab_size(ui, label);
    let response = ui
        .push_id(key, |ui| {
            ui.add_enabled(
                enabled,
                egui::Button::selectable(
                    selected,
                    RichText::new(label).color(if contextual {
                        colors.accent
                    } else {
                        colors.ink
                    }),
                )
                .fill(egui::Color32::TRANSPARENT)
                .frame(false)
                .min_size(size),
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
