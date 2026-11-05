//! Compact document tabs retain source identity independently of their captions.
use super::{Action, DocumentView, State};
use crate::localization::tr;
use crate::theme::palette;
use crate::workspace::focus::{self, Pane};
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::workspace::ribbon::Icon;
use egui::{Frame, Id, Margin, Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

const TAB_HEIGHT: f32 = 22.0;
const MIN_TAB_WIDTH: f32 = 48.0;
const MAX_TAB_WIDTH: f32 = 220.0;
const TAB_TEXT_PADDING: f32 = 6.0;
const TAB_ICON_SIZE: f32 = 14.0;
const TAB_ICON_GAP: f32 = 4.0;
const CLOSE_ICON_SIZE: f32 = 8.0;
const CLOSE_TARGET_WIDTH: f32 = 20.0;
const SCROLLBAR_THICKNESS: f32 = 6.0;

impl State {
    pub(super) fn document_tabs(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let document = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let tabs = self.object_tabs();
        let active = self.active_object();
        let active_id = active.as_ref().map(|tab| tab.id);
        let reveal_id = ui.make_persistent_id(("active_object_tab", document));
        let previous = ui.data(|data| data.get_temp::<u64>(reveal_id));
        let reveal_active = active_id != previous;
        ui.data_mut(|data| {
            if let Some(id) = active_id {
                data.insert_temp(reveal_id, id);
            } else {
                data.remove::<u64>(reveal_id);
            }
        });
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
                            for object in tabs {
                                let selected =
                                    active.as_ref().is_some_and(|active| active.id == object.id);
                                let (title, icon) = match &object.target.kind {
                                    ObjectKind::Table { name } => {
                                        let icon = if selected && self.view == DocumentView::Design
                                        {
                                            Icon::TableEdit
                                        } else {
                                            Icon::Table
                                        };
                                        (name.clone(), icon)
                                    }
                                    ObjectKind::SavedQuery { name, .. } => {
                                        (name.clone(), Icon::Search)
                                    }
                                    ObjectKind::Sql => (tr(ui, "shell-query"), Icon::Search),
                                };
                                let id = ui.make_persistent_id((
                                    "object_tab",
                                    object.target.document_revision,
                                    object.id,
                                ));
                                let can_close = self.can_close_object(&object);
                                let can_activate = self.can_activate_object(&object);
                                let (response, close) = ui
                                    .add_enabled_ui(can_activate, |ui| {
                                        tab(ui, id, &title, icon, selected, true, can_close)
                                    })
                                    .inner;
                                if selected {
                                    focus::register(ui, Pane::Document, &response, false);
                                    if reveal_active {
                                        response.scroll_to_me(None);
                                    }
                                }
                                if close {
                                    actions.push(Action::CloseObject {
                                        tab: object.clone(),
                                    });
                                } else if response.clicked() {
                                    actions.push(Action::ActivateObject { tab: object });
                                }
                            }
                            if let Some(document_revision) = document {
                                let target = ObjectTarget {
                                    document_revision,
                                    kind: ObjectKind::Sql,
                                };
                                ui.add_space(6.0);
                                if ui
                                    .add_enabled(
                                        self.can_open_object(&target),
                                        egui::Button::image_and_text(
                                            Icon::Search.image(ui, TAB_ICON_SIZE),
                                            tr(ui, "navigation-sql-query"),
                                        )
                                        .small(),
                                    )
                                    .on_hover_text(tr(ui, "navigation-sql-query-help"))
                                    .clicked()
                                {
                                    actions.push(Action::OpenObject { target });
                                }
                            }
                        });
                    });
            });
    }
}

fn tab(
    ui: &mut Ui,
    id: Id,
    title: &str,
    icon: Icon,
    selected: bool,
    closable: bool,
    can_close: bool,
) -> (Response, bool) {
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
    let close_width = if closable { CLOSE_TARGET_WIDTH } else { 0.0 };
    let content_inset = TAB_TEXT_PADDING * 2.0 + TAB_ICON_SIZE + TAB_ICON_GAP + close_width;
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
    let close = if closable {
        let close_rect = egui::Rect::from_min_max(
            egui::pos2(rect.right() - close_width, rect.top()),
            rect.right_bottom(),
        );
        let close = ui
            .add_enabled_ui(can_close, |ui| {
                ui.interact(close_rect, id.with("close"), Sense::click())
            })
            .inner;
        let label = tr(ui, "navigation-close-object");
        close.widget_info(|| WidgetInfo::labeled(WidgetType::Button, close.enabled(), &label));
        ui.ctx().accesskit_node_builder(close.id, |node| {
            node.set_description(title);
            if !close.enabled() {
                node.remove_action(egui::accesskit::Action::Click);
                node.remove_action(egui::accesskit::Action::Focus);
            }
        });
        let color = if close.enabled() {
            colors.ink
        } else {
            colors.muted
        };
        let glyph =
            egui::Rect::from_center_size(close_rect.center(), egui::Vec2::splat(CLOSE_ICON_SIZE));
        let painter = ui
            .painter()
            .with_clip_rect(close_rect.intersect(ui.clip_rect()));
        painter.line_segment(
            [glyph.left_top(), glyph.right_bottom()],
            egui::Stroke::new(1.0, color),
        );
        painter.line_segment(
            [glyph.right_top(), glyph.left_bottom()],
            egui::Stroke::new(1.0, color),
        );
        if close.has_focus() {
            painter.rect_stroke(
                close_rect,
                0.0,
                egui::Stroke::new(1.0, colors.accent),
                egui::StrokeKind::Inside,
            );
        }
        if close.gained_focus() {
            close.scroll_to_me(None);
        }
        close.on_hover_text(label).clicked()
    } else {
        false
    };
    (response.on_hover_text(title), close)
}

#[cfg(test)]
mod tests;
