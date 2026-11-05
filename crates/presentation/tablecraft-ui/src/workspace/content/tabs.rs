//! Compact document tabs retain source identity independently of their captions.
use super::{DocumentView, State};
use crate::localization::tr;
use crate::theme::palette;
use crate::workspace::focus::{self, Pane};
use crate::workspace::ribbon::Icon;
use egui::{Frame, Id, Margin, Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

const TAB_HEIGHT: f32 = 22.0;
const MIN_TAB_WIDTH: f32 = 48.0;
const MAX_TAB_WIDTH: f32 = 220.0;
const TAB_TEXT_PADDING: f32 = 6.0;
const TAB_ICON_SIZE: f32 = 14.0;
const TAB_ICON_GAP: f32 = 4.0;
const SCROLLBAR_THICKNESS: f32 = 6.0;

impl State {
    pub(super) fn document_tabs(&mut self, ui: &mut Ui) {
        let document = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let table = self.data.as_ref().map(|data| data.table.as_str());
        let saved_query = self
            .saved_query
            .active
            .then_some(self.saved_query.target.as_ref())
            .flatten();
        let table_title = table
            .map(str::to_owned)
            .unwrap_or_else(|| tr(ui, "shell-table"));
        let mut selected_view = None;

        Frame::new()
            .inner_margin(Margin {
                left: 4,
                right: 4,
                top: 1,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                ui.spacing_mut().scroll.bar_width = SCROLLBAR_THICKNESS;
                egui::ScrollArea::horizontal()
                    .id_salt(("document_tabs", document))
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            for (view, title, icon, id) in [
                                (
                                    DocumentView::Table,
                                    table_title,
                                    Icon::Table,
                                    ui.make_persistent_id(("table_tab", document, table)),
                                ),
                                (
                                    DocumentView::Query,
                                    tr(ui, "shell-query"),
                                    Icon::Search,
                                    ui.make_persistent_id((
                                        "query_tab",
                                        document,
                                        saved_query.map(|target| {
                                            (target.document_revision, target.object_id)
                                        }),
                                    )),
                                ),
                                (
                                    DocumentView::Design,
                                    tr(ui, "shell-table-design"),
                                    Icon::TableEdit,
                                    ui.make_persistent_id((
                                        "design_tab",
                                        document,
                                        self.schema.as_ref().map(|schema| schema.table.as_str()),
                                    )),
                                ),
                            ] {
                                if view == DocumentView::Design && self.schema.is_none() {
                                    continue;
                                }
                                let response = tab(ui, id, &title, icon, self.view == view);
                                if self.view == view {
                                    focus::register(ui, Pane::Document, &response, false);
                                }
                                if response.clicked() {
                                    selected_view = Some(view);
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                    });
            });
        if let Some(view) = selected_view {
            self.select_view(view);
        }
    }
}

fn tab(ui: &mut Ui, id: Id, title: &str, icon: Icon, selected: bool) -> Response {
    let colors = palette(ui.ctx());
    let color = if ui.is_enabled() {
        colors.ink
    } else {
        colors.muted
    };
    let font = ui.style().text_styles[&TextStyle::Button].clone();
    let natural_width = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font.clone(), color)
        .size()
        .x;
    let content_inset = TAB_TEXT_PADDING * 2.0 + TAB_ICON_SIZE + TAB_ICON_GAP;
    let width = (natural_width + content_inset).clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH);
    let (_, rect) = ui.allocate_space(egui::vec2(width, TAB_HEIGHT));
    let response = ui.interact(rect, id, Sense::click());

    let fill = if selected {
        colors.title_fill
    } else if response.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        colors.surface
    };
    ui.painter().rect(
        rect,
        0.0,
        fill,
        egui::Stroke::new(1.0, colors.grid_line),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    let mut layout =
        egui::text::LayoutJob::simple(title.to_owned(), font, color, width - content_inset);
    layout.wrap.max_rows = 1;
    layout.wrap.break_anywhere = true;
    let text = ui.painter().layout_job(layout);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + TAB_TEXT_PADDING,
            rect.center().y - TAB_ICON_SIZE * 0.5,
        ),
        egui::vec2(TAB_ICON_SIZE, TAB_ICON_SIZE),
    );
    icon.image(ui, TAB_ICON_SIZE).paint_at(ui, icon_rect);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(
            egui::pos2(
                icon_rect.right() + TAB_ICON_GAP,
                rect.center().y - text.size().y * 0.5,
            ),
            text,
            color,
        );
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, title));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Tab);
        node.clear_toggled();
        node.set_selected(selected);
        if !ui.is_enabled() {
            node.remove_action(egui::accesskit::Action::Click);
            node.remove_action(egui::accesskit::Action::Focus);
        }
    });
    if response.clicked() {
        response.request_focus();
    }
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    response.on_hover_text(title)
}

#[cfg(test)]
mod tests;
