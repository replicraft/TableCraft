//! Session-only Navigation Pane heading and category presentation.
use super::super::focus::{self, Pane};
use crate::{fonts::bold_family, localization::tr};
use egui::{
    Align, Id, Layout, Response, RichText, Sense, Stroke, TextStyle, Ui, WidgetInfo, WidgetType,
};

const HEADER_HEIGHT: f32 = 22.0;
const CHEVRON_PADDING: f32 = 9.0;
const CHEVRON_HALF_WIDTH: f32 = 3.0;
const COLLAPSE_SIZE: f32 = 18.0;
const PANE_STRIP_MARGIN: i8 = 3;
const PANE_STRIP_TOP: i8 = 5;
const PANE_STRIP_WIDTH: f32 = COLLAPSE_SIZE + 2.0 * PANE_STRIP_MARGIN as f32;
const PANE_CHEVRON_HALF_WIDTH: f32 = 2.0;
const PANE_CHEVRON_HALF_HEIGHT: f32 = 4.0;
const TABLES_EXPANDED: &str = "navigation_tables_expanded";
const QUERIES_EXPANDED: &str = "navigation_queries_expanded";

/// Presentation state participates in pointer activation identity; hiding and
/// restoring a group cannot reuse a click made before that transition.
#[derive(PartialEq, Eq)]
pub(super) struct Expansion {
    tables: bool,
    queries: bool,
}

pub(super) fn expansion(ui: &Ui) -> Expansion {
    ui.ctx().data(|data| Expansion {
        tables: data
            .get_temp::<bool>(Id::new(TABLES_EXPANDED))
            .unwrap_or(true),
        queries: data
            .get_temp::<bool>(Id::new(QUERIES_EXPANDED))
            .unwrap_or(true),
    })
}

pub(super) fn heading(ui: &mut Ui, enabled: bool) -> bool {
    ui.horizontal(|ui| {
        let heading = tr(ui, "navigation-all-objects");
        let label_width =
            (ui.available_width() - COLLAPSE_SIZE - ui.spacing().item_spacing.x).max(0.0);
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, HEADER_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_size(egui::vec2(label_width, HEADER_HEIGHT));
                ui.add(
                    egui::Label::new(RichText::new(&heading).strong().size(15.0))
                        .truncate()
                        .halign(Align::Min),
                )
                .on_hover_text(heading);
            },
        );
        pane_button(ui, enabled, true)
    })
    .inner
}

/// Mouse restoration is a TableCraft adaptation until a corresponding collapsed
/// Access Navigation Pane reference has been retained.
pub(super) fn collapsed(ui: &mut Ui, enabled: bool) -> bool {
    egui::Panel::left("navigation_collapsed")
        .exact_size(PANE_STRIP_WIDTH)
        .resizable(false)
        .frame(
            egui::Frame::side_top_panel(ui.style())
                .inner_margin(egui::Margin::symmetric(PANE_STRIP_MARGIN, PANE_STRIP_TOP)),
        )
        .show(ui, |ui| pane_button(ui, enabled, false))
        .inner
}

fn pane_button(ui: &mut Ui, enabled: bool, collapse: bool) -> bool {
    ui.add_enabled_ui(enabled, |ui| {
        let label = tr(
            ui,
            if collapse {
                "navigation-collapse-pane"
            } else {
                "navigation-expand-pane"
            },
        );
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(COLLAPSE_SIZE), Sense::click());
        let colors = crate::theme::palette(ui.ctx());
        if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(rect, 2.0, colors.active_header);
        }
        let center = rect.center();
        let direction = if collapse { -1.0 } else { 1.0 };
        ui.painter().line(
            vec![
                center
                    + egui::vec2(
                        -direction * PANE_CHEVRON_HALF_WIDTH,
                        -PANE_CHEVRON_HALF_HEIGHT,
                    ),
                center + egui::vec2(direction * PANE_CHEVRON_HALF_WIDTH, 0.0),
                center
                    + egui::vec2(
                        -direction * PANE_CHEVRON_HALF_WIDTH,
                        PANE_CHEVRON_HALF_HEIGHT,
                    ),
            ],
            Stroke::new(
                1.2,
                if ui.is_enabled() {
                    colors.ink
                } else {
                    colors.muted
                },
            ),
        );
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
        // A collapsed strip keeps its ordinary Tab-accessible button without
        // restoring the hidden pane to the workspace's F6 pane cycle.
        if collapse {
            focus::register(ui, Pane::Navigation, &response, false);
        }
        response.on_hover_text(label).clicked()
    })
    .inner
}

pub(super) fn tables(ui: &mut Ui) -> bool {
    category(ui, Id::new(TABLES_EXPANDED), "navigation-tables")
}

pub(super) fn queries(ui: &mut Ui) -> bool {
    category(ui, Id::new(QUERIES_EXPANDED), "navigation-queries")
}

fn category(ui: &mut Ui, id: Id, message: &str) -> bool {
    let mut expanded = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(id))
        .unwrap_or(true);
    let label = tr(ui, message);
    let response = header(ui, &label, expanded);
    if response.clicked() {
        expanded = !expanded;
        ui.ctx().data_mut(|data| data.insert_temp(id, expanded));
    }
    focus::register(ui, Pane::Navigation, &response, false);
    expanded
}

fn header(ui: &mut Ui, label: &str, expanded: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HEADER_HEIGHT),
        Sense::click(),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_expanded(expanded);
    });
    let colors = crate::theme::palette(ui.ctx());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 0.0, colors.active_header);
    }
    let text_width = (rect.width() - 2.0 - COLLAPSE_SIZE).max(0.0);
    let galley = egui::WidgetText::from(RichText::new(label).family(bold_family(ui)).strong())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            text_width,
            TextStyle::Button,
        );
    ui.painter().galley(
        egui::pos2(rect.left() + 2.0, rect.center().y - galley.size().y * 0.5),
        galley,
        colors.ink,
    );
    let center = egui::pos2(rect.right() - CHEVRON_PADDING, rect.center().y);
    let direction = if expanded { -1.0 } else { 1.0 };
    ui.painter().line(
        vec![
            center + egui::vec2(-CHEVRON_HALF_WIDTH, -direction),
            center + egui::vec2(0.0, direction * 2.0),
            center + egui::vec2(CHEVRON_HALF_WIDTH, -direction),
        ],
        Stroke::new(1.0, colors.muted),
    );
    response
}
