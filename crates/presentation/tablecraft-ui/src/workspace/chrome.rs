//! Shared title, contextual ribbon and status; no file or database services.
mod quick_access;
pub(super) mod ribbon_display;
mod window;
use super::focus::{self, Pane};
use super::objects::ObjectKind;
use super::ribbon::Icon;
use super::{Action, DocumentView, State, ribbon};
use crate::theme::palette;
use egui::{Frame, Margin, RichText, Ui, WidgetInfo, WidgetType};

const APPLICATION_ICON_SIZE: f32 = 16.0;
const RIBBON_TAB_HEIGHT: f32 = 26.0;
const RIBBON_TAB_SIDE_PADDING: f32 = 9.0;
const ZOOM_STEP_PERCENT: u16 = 10;
const ZOOM_SLIDER_WIDTH: f32 = 96.0;
const STATUS_VIEW_ICON_SIZE: f32 = 16.0;

impl State {
    fn ribbon_tab_available(&self, candidate: ribbon::Tab) -> bool {
        match candidate {
            ribbon::Tab::QueryDesign => self.query_design_context(),
            ribbon::Tab::TableDesign => {
                !self.query_design_context() && self.view == DocumentView::Design
            }
            ribbon::Tab::TableFields | ribbon::Tab::Table => {
                !self.query_design_context()
                    && self.view == DocumentView::Table
                    && self.data.is_some()
            }
            _ => true,
        }
    }

    pub(super) fn chrome(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        if !self.ribbon_tab_available(self.ribbon_tab) {
            self.ribbon_tab = ribbon::Tab::Home;
            self.set_ribbon_collapsed(self.ribbon_collapsed());
        }
        let colors = palette(ui.ctx());
        let t = self.localizer.clone();
        egui::Panel::top("title")
            .frame(Frame::new().fill(colors.title_fill).inner_margin(Margin {
                left: 8,
                right: if window::enabled() { 0 } else { 8 },
                top: if window::enabled() { 0 } else { 4 },
                bottom: if window::enabled() { 0 } else { 4 },
            }))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        Icon::Application
                            .image(ui, APPLICATION_ICON_SIZE)
                            .alt_text(t.text("shell-app-name")),
                    )
                    .on_hover_text(t.text("shell-app-name"));
                    ui.label(RichText::new(t.text("shell-app-name")).strong());
                    if !self.backstage_visible() {
                        ui.add_space(8.0);
                        ui.separator();
                        let open_available = !self.busy
                            && !self.edit_dialog_open()
                            && !self.saved_query_prompt_open();
                        if quick_access::show(
                            ui,
                            &t,
                            open_available,
                            self.can_save_table_draft(),
                            actions,
                        ) {
                            self.request_table_draft_save();
                        }
                        ui.add_space(12.0);
                    }
                    let title = self
                        .database
                        .as_ref()
                        .map_or_else(String::new, |db| db.title.clone());
                    let title_hover = self
                        .database
                        .as_ref()
                        .and_then(|database| database.title_path.as_deref())
                        .unwrap_or(&title);
                    let status = if self.field_filter_dialog_open() {
                        Some(t.text("filtering-field-title"))
                    } else if self.edit_dialog_open() {
                        Some(self.edit_status())
                    } else if self.busy {
                        Some(t.text("shell-working"))
                    } else {
                        None
                    };
                    let status =
                        status.map(|text| RichText::new(text).color(colors.muted).size(11.0));
                    let status_width = status.as_ref().map_or(0.0, |text| {
                        egui::WidgetText::from(text.clone())
                            .into_galley(
                                ui,
                                Some(egui::TextWrapMode::Extend),
                                f32::INFINITY,
                                egui::TextStyle::Body,
                            )
                            .size()
                            .x
                            + ui.spacing().item_spacing.x
                    });
                    let controls_width = if window::enabled() {
                        window::CONTROLS_WIDTH + ui.spacing().item_spacing.x
                    } else {
                        0.0
                    };
                    let title_width =
                        (ui.available_width() - status_width - controls_width).max(0.0);
                    let response = ui
                        .allocate_ui_with_layout(
                            egui::vec2(title_width, ui.text_style_height(&egui::TextStyle::Body)),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown)
                                .with_cross_align(egui::Align::LEFT),
                            |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(&title).color(colors.ink))
                                        .sense(if window::enabled() {
                                            egui::Sense::CLICK | egui::Sense::DRAG
                                        } else {
                                            egui::Sense::hover()
                                        })
                                        .halign(egui::Align::LEFT)
                                        .truncate(),
                                )
                            },
                        )
                        .inner
                        .on_hover_text(title_hover);
                    window::drag_title(&response);
                    ui.ctx().accesskit_node_builder(response.id, |node| {
                        node.set_label(title);
                    });
                    if let Some(status) = status {
                        ui.label(status);
                    }
                    if window::enabled() {
                        window::show(ui, &t);
                    }
                });
            });
        if !self.backstage_visible() {
            let tabs = egui::Panel::top("ribbon_tabs")
                .frame(Frame::new().fill(colors.title_fill).inner_margin(Margin {
                    left: 8,
                    right: if self.ribbon_collapsed() {
                        ribbon_display::TOGGLE_GUTTER
                    } else {
                        8
                    },
                    top: 0,
                    bottom: 0,
                }))
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
                                ribbon::keyboard::tab(ui, &file, None);
                                if file.clicked() {
                                    self.show_open();
                                }
                                let response = tab(
                                    ui,
                                    "ribbon-home",
                                    &t.text("ribbon-home"),
                                    self.ribbon_tab == ribbon::Tab::Home,
                                    true,
                                    false,
                                );
                                self.ribbon_tab_response(ui, &response, ribbon::Tab::Home);
                                for key in [
                                    "ribbon-create",
                                    "ribbon-external-data",
                                    "ribbon-database-tools",
                                    "ribbon-help",
                                ] {
                                    let candidate =
                                        ribbon::Tab::from_key(key).expect("Ribbon tab key");
                                    let response = tab(
                                        ui,
                                        key,
                                        &t.text(key),
                                        self.ribbon_tab == candidate,
                                        true,
                                        false,
                                    );
                                    self.ribbon_tab_response(ui, &response, candidate);
                                }
                                if self.query_design_context() {
                                    let response = tab(
                                        ui,
                                        "qbe-ribbon-design",
                                        &t.text("qbe-ribbon-design"),
                                        self.ribbon_tab == ribbon::Tab::QueryDesign,
                                        true,
                                        true,
                                    );
                                    self.ribbon_tab_response(
                                        ui,
                                        &response,
                                        ribbon::Tab::QueryDesign,
                                    );
                                } else if self.ribbon_tab_available(ribbon::Tab::TableDesign) {
                                    let response = tab(
                                        ui,
                                        "ribbon-table-design",
                                        &t.text("ribbon-table-design"),
                                        self.ribbon_tab == ribbon::Tab::TableDesign,
                                        true,
                                        true,
                                    );
                                    self.ribbon_tab_response(
                                        ui,
                                        &response,
                                        ribbon::Tab::TableDesign,
                                    );
                                } else if self.ribbon_tab_available(ribbon::Tab::TableFields) {
                                    for key in ["ribbon-table-fields", "ribbon-table"] {
                                        let candidate = ribbon::Tab::from_key(key)
                                            .expect("contextual Ribbon tab key");
                                        let response = tab(
                                            ui,
                                            key,
                                            &t.text(key),
                                            self.ribbon_tab == candidate,
                                            true,
                                            true,
                                        );
                                        self.ribbon_tab_response(ui, &response, candidate);
                                    }
                                }
                            });
                        });
                });
            self.finish_ribbon_tab_clicks(ui);
            self.collapsed_ribbon_toggle(ui, tabs.response.rect);
            self.show_ribbon_commands(ui, tabs.response.rect, actions);
        }
        egui::Panel::bottom("status")
            .frame(
                Frame::new()
                    .fill(colors.title_fill)
                    .inner_margin(Margin::symmetric(8, 3)),
            )
            .show(ui, |ui| self.status_bar(ui, actions));
    }
}

impl State {
    fn status_bar(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        let colors = palette(ui.ctx());
        ui.horizontal(|ui| {
            let key = if self.database.is_none() {
                "shell-ready"
            } else {
                match self.view {
                    DocumentView::Table => "shell-view-datasheet",
                    DocumentView::Design => "shell-view-design",
                    DocumentView::Query => "shell-view-query",
                }
            };
            ui.small(t.text(key));
            if let Some(key) = self.save_cleanup_status() {
                ui.separator();
                ui.small(t.text(key));
            }
            if self.busy {
                ui.spinner();
                if ui.button(t.text("shell-cancel")).clicked() {
                    actions.push(Action::Cancel);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let zoom_available = ui.is_enabled()
                    && !self.backstage_visible()
                    && self
                        .visible_data()
                        .is_some_and(|data| !data.columns.is_empty());
                if zoom_available {
                    ui.scope(|ui| {
                        let mut zoom = crate::theme::datasheet_zoom(ui.ctx());
                        ui.small(format!("{zoom}%"));
                        let increase = ui.add(egui::Button::new("+").frame(false));
                        increase.widget_info(|| {
                            WidgetInfo::labeled(
                                WidgetType::Button,
                                increase.enabled(),
                                t.text("shell-zoom-in"),
                            )
                        });
                        if increase.on_hover_text(t.text("shell-zoom-in")).clicked() {
                            zoom = zoom.saturating_add(ZOOM_STEP_PERCENT);
                        }
                        let mut style = (**ui.style()).clone();
                        style.spacing.slider_width = ZOOM_SLIDER_WIDTH;
                        style.spacing.slider_rail_height = 2.0;
                        style.spacing.interact_size.y = 14.0;
                        for widget in [
                            &mut style.visuals.widgets.inactive,
                            &mut style.visuals.widgets.hovered,
                            &mut style.visuals.widgets.active,
                        ] {
                            widget.corner_radius = egui::CornerRadius::ZERO;
                            widget.bg_fill = colors.muted;
                            widget.fg_stroke = egui::Stroke::NONE;
                        }
                        ui.scope(|ui| {
                            ui.set_style(style);
                            let slider = ui.add(
                                egui::Slider::new(
                                    &mut zoom,
                                    crate::theme::MIN_DATASHEET_ZOOM
                                        ..=crate::theme::MAX_DATASHEET_ZOOM,
                                )
                                .show_value(false)
                                .logarithmic(true)
                                .trailing_fill(false)
                                .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.2 }),
                            );
                            slider.widget_info(|| {
                                WidgetInfo::labeled(
                                    WidgetType::Slider,
                                    slider.enabled(),
                                    t.text("shell-zoom"),
                                )
                            });
                        });
                        let decrease = ui.add(egui::Button::new("−").frame(false));
                        decrease.widget_info(|| {
                            WidgetInfo::labeled(
                                WidgetType::Button,
                                decrease.enabled(),
                                t.text("shell-zoom-out"),
                            )
                        });
                        if decrease.on_hover_text(t.text("shell-zoom-out")).clicked() {
                            zoom = zoom.saturating_sub(ZOOM_STEP_PERCENT);
                        }
                        if zoom != crate::theme::datasheet_zoom(ui.ctx()) {
                            crate::theme::set_datasheet_zoom(ui.ctx(), zoom);
                        }
                    });
                }
                let active = self.active_object();
                let table = active.as_ref().and_then(|tab| match &tab.target.kind {
                    ObjectKind::Table { name } => Some(name.clone()),
                    _ => None,
                });
                let design_available = ui.is_enabled()
                    && self.view != DocumentView::Design
                    && active
                        .as_ref()
                        .is_some_and(|tab| self.can_view_object_design(tab));
                let datasheet_available = ui.is_enabled()
                    && self.view != DocumentView::Table
                    && active
                        .as_ref()
                        .is_some_and(|tab| self.can_view_object_datasheet(tab));
                if zoom_available && (design_available || datasheet_available) {
                    ui.separator();
                }
                if design_available {
                    let design = ui.add(
                        egui::Button::image(Icon::ViewDesign.image(ui, STATUS_VIEW_ICON_SIZE))
                            .frame(false),
                    );
                    design.widget_info(|| {
                        WidgetInfo::labeled(
                            WidgetType::Button,
                            design.enabled(),
                            t.text("shell-view-design"),
                        )
                    });
                    if design.clicked() && self.view != DocumentView::Design {
                        actions.push(Action::ViewSchema {
                            table: table.clone().expect("enabled table view"),
                        });
                    }
                }
                if datasheet_available {
                    let datasheet = ui.add(
                        egui::Button::image(Icon::ViewDatasheet.image(ui, STATUS_VIEW_ICON_SIZE))
                            .frame(false),
                    );
                    datasheet.widget_info(|| {
                        WidgetInfo::labeled(
                            WidgetType::Button,
                            datasheet.enabled(),
                            t.text("shell-view-datasheet"),
                        )
                    });
                    if datasheet.clicked()
                        && let Some(tab) = active
                    {
                        actions.push(Action::ViewObjectDatasheet { tab });
                    }
                }
            });
        });
    }
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
