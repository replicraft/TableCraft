mod selection;
pub(crate) use selection::Selection;

use super::{Action, Cell, Data};
use crate::theme::{GRID_HEADER, GRID_LINE};
use egui::{Response, RichText, ScrollArea, Sense, Ui, WidgetInfo, WidgetType};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;

const ACTIVE_HEADER: egui::Color32 = egui::Color32::from_rgb(255, 225, 102);
const ACTIVE_ROW: egui::Color32 = egui::Color32::from_rgb(166, 205, 234);
const CELL_TEXT_PADDING: f32 = 3.0;
const BOOLEAN_BOX_SIZE: f32 = 14.0;

pub(super) fn show(ui: &mut Ui, data: &Data, selection: &mut Selection) -> Vec<Action> {
    let mut actions = Vec::new();
    show_rows(ui, data, |_| {}, &mut actions, selection);
    actions
}

fn show_rows(
    ui: &mut Ui,
    data: &Data,
    mut row_drawn: impl FnMut(usize),
    actions: &mut Vec<Action>,
    selection: &mut Selection,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    selection.prepare(ui, data);
    let scroll_to = selection.scroll_to_row();
    ui.push_id((&data.table, data.offset), |ui| {
        // Reserve scrollbar space below the viewport, including one-row pages.
        // The style change is scoped to this datasheet and its nested table.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        ScrollArea::horizontal()
            .id_salt("datasheet_horizontal")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut table = TableBuilder::new(ui);
                if let Some(row) = scroll_to {
                    table = table.scroll_to_row(row, Some(egui::Align::Center));
                }
                table
                    .auto_shrink([false, false])
                    .striped(false)
                    .resizable(true)
                    .column(Column::exact(28.0))
                    .columns(Column::initial(128.0).at_least(60.0), data.columns.len())
                    .header(22.0, |mut header| {
                        header.col(|ui| {
                            grid_cell(ui, true);
                            ui.label(RichText::new("#"));
                        });
                        for (column, name) in data.columns.iter().enumerate() {
                            header.col(|ui| {
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.column() == Some(column) {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ACTIVE_HEADER);
                                }
                                ui.label(RichText::new(name));
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(20.0, data.rows.len(), |mut row| {
                            let index = row.index();
                            row_drawn(index);
                            let number = u64::from(data.offset) + index as u64 + 1;
                            row.col(|ui| {
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.row() == Some(index) {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ACTIVE_HEADER);
                                }
                                ui.weak(number.to_string());
                            });
                            for (position, (column, text)) in
                                data.columns.iter().zip(&data.rows[index]).enumerate()
                            {
                                row.col(|ui| {
                                    grid_cell(ui, false);
                                    let active = selection.selected(index, position);
                                    if ui.is_enabled() && selection.row() == Some(index) {
                                        let fill = if active {
                                            egui::Color32::WHITE
                                        } else {
                                            ACTIVE_ROW
                                        };
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, fill);
                                    }
                                    let response = cell(ui, text, column, number, active);
                                    selection.observe(ui, index, position, &response);
                                    if ui.is_enabled() && selection.selected(index, position) {
                                        ui.painter().rect_stroke(
                                            ui.max_rect(),
                                            0.0,
                                            egui::Stroke::new(1.0, crate::theme::ACCENT),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    collect_contents_action(
                                        ui,
                                        &response,
                                        data,
                                        (index, position),
                                        actions,
                                    );
                                });
                            }
                        });
                    });
                // The inner vertical ScrollArea consumes both scroll targets,
                // including its disabled horizontal axis. Issue this one-shot
                // cell reveal only after that inner area has completed.
                if let Some(rect) = selection.take_reveal() {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
            })
    })
    .inner
}

fn collect_contents_action(
    ui: &mut Ui,
    response: &Response,
    data: &Data,
    position: (usize, usize),
    actions: &mut Vec<Action>,
) {
    let (row, column) = position;
    let Some(value) = data.rows.get(row).and_then(|row| row.get(column)) else {
        return;
    };
    if data.source_revision.is_some()
        && !matches!(value.kind, crate::CellKind::Scalar { .. })
        && contents_requested(ui, response)
        && let Some(target) = data.contents_target(row, column)
    {
        actions.push(Action::ViewContents { target });
    }
}

fn contents_requested(ui: &mut Ui, response: &Response) -> bool {
    if !ui.is_enabled() {
        return false;
    }
    let mut requested = false;
    response.context_menu(|ui| {
        if ui.button("View contents").clicked() {
            requested = true;
            ui.close();
        }
    });
    if response.has_focus() {
        requested |= ui.input_mut(|input| {
            let mut enter = false;
            input.events.retain(|event| {
                let matches = matches!(event, egui::Event::Key {
                    key: egui::Key::Enter, pressed: true, modifiers, ..
                } if modifiers.is_none());
                enter |= matches;
                !matches
            });
            enter
        });
    }
    requested
}

fn grid_cell(ui: &Ui, header: bool) {
    let rect = ui.max_rect();
    if header {
        ui.painter().rect_filled(rect, 0.0, GRID_HEADER);
    }
    let stroke = egui::Stroke::new(0.5, GRID_LINE);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
}

fn cell(ui: &mut Ui, value: &Cell, column: &str, row: u64, allow_copy: bool) -> Response {
    let text = value.text.as_str();
    let context = value.context();
    let display = match value.badge() {
        Some(badge) => Cow::Owned(format!("{text}  [{badge}]")),
        None => Cow::Borrowed(text),
    };
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    let checkbox = value.checkbox_value();
    if let Some(checked) = checkbox {
        paint_boolean(ui, response.rect, checked, value.badge());
    } else {
        paint_cell_text(ui, response.rect, display.as_ref());
    }
    if response.clicked() {
        response.request_focus();
    }
    if ui.is_enabled() && response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
    }
    response.widget_info(|| {
        let label = match &context {
            Some(context) => format!("{column}, row {row}: {text}; {context}"),
            None => format!("{column}, row {row}: {text}"),
        };
        if let Some(checked) = checkbox {
            WidgetInfo::selected(WidgetType::Checkbox, ui.is_enabled(), checked, label)
        } else {
            WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), label)
        }
    });
    if checkbox.is_some() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_read_only();
            // This response selects a cell. It never offers an assistive
            // action that could be understood as toggling the stored value.
            node.remove_action(egui::accesskit::Action::Click);
        });
    }

    if allow_copy && response.has_focus() && ui.is_enabled() {
        let copy = ui.input_mut(|input| {
            let copy = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Copy));
            if copy {
                input
                    .events
                    .retain(|event| !matches!(event, egui::Event::Copy));
            }
            copy
        });
        if copy {
            copy_text(ui.ctx(), text);
        }
        egui::Tooltip::for_widget(&response).show(|ui| {
            ui.label(text);
            if let Some(context) = &context {
                ui.small(context);
            }
            ui.small("Copy copies the full cell value.");
        });
    }
    response.on_hover_ui(|ui| {
        ui.label(text);
        if let Some(context) = &context {
            ui.small(context);
        }
    })
}

fn paint_cell_text(ui: &Ui, rect: egui::Rect, text: &str) {
    let width = (rect.width() - 2.0 * CELL_TEXT_PADDING).max(0.0);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width,
        egui::TextStyle::Body,
    );
    let position = egui::pos2(
        rect.left() + CELL_TEXT_PADDING,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter()
        .galley(position, galley, ui.visuals().text_color());
}

fn paint_boolean(ui: &Ui, rect: egui::Rect, checked: bool, badge: Option<&str>) {
    let size = BOOLEAN_BOX_SIZE.min(rect.height());
    let check = egui::Rect::from_center_size(
        egui::pos2(
            rect.left() + CELL_TEXT_PADDING + size * 0.5,
            rect.center().y,
        ),
        egui::Vec2::splat(size),
    );
    let color = egui::Color32::from_rgb(44, 107, 159);
    let fill = if checked { color } else { egui::Color32::WHITE };
    ui.painter().rect_filled(check, 2.0, fill);
    ui.painter().rect_stroke(
        check,
        2.0,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
    if checked {
        let inner = check.shrink(3.0);
        ui.painter().add(egui::epaint::Shape::line(
            vec![
                inner.left_center(),
                egui::pos2(inner.center().x - 1.0, inner.bottom()),
                inner.right_top(),
            ],
            egui::Stroke::new(1.5, egui::Color32::WHITE),
        ));
    }
    if let Some(badge) = badge {
        let badge_rect =
            egui::Rect::from_min_max(egui::pos2(check.right() + 3.0, rect.top()), rect.max);
        paint_cell_text(ui, badge_rect, &format!("[{badge}]"));
    }
}

pub(crate) fn copy_text(ctx: &egui::Context, text: &str) {
    ctx.copy_text(text.to_owned());
}

#[cfg(test)]
mod tests;
