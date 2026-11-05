mod clipboard;
mod collection_columns;
mod preview;
mod selection;
pub(crate) use selection::Selection;

use super::{Action, Cell, Data, HorizontalAlignment};
use crate::localization::{for_context, message, tr};
use crate::workspace::focus::{self, Pane};
use egui::{Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetType};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;

const RECORD_SELECTOR_WIDTH: f32 = 18.0;
const CELL_TEXT_PADDING: f32 = 3.0;
const BOOLEAN_BOX_SIZE: f32 = 14.0;
const HEADER_ROW_HEIGHT: f32 = 22.0;
const DATA_ROW_HEIGHT: f32 = 20.0;
const SORT_CONTROL_WIDTH: f32 = 18.0;
const SORT_CHEVRON_HALF_WIDTH: f32 = 3.0;

struct RowView {
    collection: bool,
    pane: Pane,
    sort_revision: Option<u64>,
}

pub(super) fn show(
    ui: &mut Ui,
    data: &Data,
    selection: &mut Selection,
    pane: Pane,
    sort_revision: Option<u64>,
) -> Vec<Action> {
    let mut actions = Vec::new();
    if let Some(error) = selection.copy_error(ui.ctx()) {
        ui.colored_label(crate::theme::palette(ui.ctx()).accent, error);
    }
    render_rows(
        ui,
        data,
        |_| {},
        &mut actions,
        selection,
        RowView {
            collection: false,
            pane,
            sort_revision,
        },
    );
    actions
}

/// Collection metadata has a bounded dialog viewport, so its final field can
/// use the remaining width without changing native table column proportions.
pub(crate) fn show_collection(ui: &mut Ui, data: &Data, selection: &mut Selection) {
    if let Some(error) = selection.copy_error(ui.ctx()) {
        ui.colored_label(crate::theme::palette(ui.ctx()).accent, error);
    }
    render_rows(
        ui,
        data,
        |_| {},
        &mut Vec::new(),
        selection,
        RowView {
            collection: true,
            pane: Pane::Document,
            sort_revision: None,
        },
    );
}

#[cfg(test)]
fn show_rows(
    ui: &mut Ui,
    data: &Data,
    row_drawn: impl FnMut(usize),
    actions: &mut Vec<Action>,
    selection: &mut Selection,
    pane: Pane,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    render_rows(
        ui,
        data,
        row_drawn,
        actions,
        selection,
        RowView {
            collection: false,
            pane,
            sort_revision: None,
        },
    )
}

fn render_rows(
    ui: &mut Ui,
    data: &Data,
    mut row_drawn: impl FnMut(usize),
    actions: &mut Vec<Action>,
    selection: &mut Selection,
    view: RowView,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    let collection_layout = view.collection;
    let pane = view.pane;
    selection.prepare(ui, data);
    let scroll_to = selection.scroll_to_row();
    ui.push_id((&data.table, data.offset), |ui| {
        let scale = crate::theme::datasheet_scale(ui.ctx());
        for text_style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            if let Some(font) = ui.style_mut().text_styles.get_mut(&text_style) {
                font.size *= scale;
            }
        }
        ui.spacing_mut().icon_width = BOOLEAN_BOX_SIZE * scale;
        // Reserve scrollbar space below the viewport, including one-row pages.
        // The style change is scoped to this datasheet and its nested table.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ScrollArea::horizontal()
            .id_salt("datasheet_horizontal")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let initial_widths =
                    collection_layout.then(|| collection_columns::initial_widths(ui, data));
                let mut table = TableBuilder::new(ui);
                if let Some(row) = scroll_to {
                    table = table.scroll_to_row(row, Some(egui::Align::Center));
                }
                table = table
                    .auto_shrink([false, false])
                    .striped(false)
                    .resizable(true)
                    .column(Column::exact(RECORD_SELECTOR_WIDTH * scale));
                for index in 0..data.columns.len() {
                    let column = if collection_layout && index + 1 == data.columns.len() {
                        Column::remainder().at_least(128.0)
                    } else {
                        let width = initial_widths
                            .as_ref()
                            .map_or(128.0, |widths| widths[index]);
                        Column::initial(width).at_least(60.0)
                    };
                    table = table.column(column);
                }
                table
                    .header(HEADER_ROW_HEIGHT * scale, |mut header| {
                        header.col(|ui| {
                            grid_cell(ui, true);
                        });
                        for (index, column) in data.columns.iter().enumerate() {
                            header.col(|ui| {
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.column() == Some(index) {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        crate::theme::palette(ui.ctx()).active_header,
                                    );
                                }
                                let label =
                                    column_header(ui, data, index, view.sort_revision, actions);
                                let description = message(
                                    ui,
                                    "datasheet-field-name",
                                    &[("name", column.name.clone())],
                                );
                                ui.ctx().accesskit_node_builder(label.id, |node| {
                                    node.set_description(description);
                                });
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(DATA_ROW_HEIGHT * scale, data.rows.len(), |mut row| {
                            let index = row.index();
                            row_drawn(index);
                            let number = u64::from(data.offset) + index as u64 + 1;
                            row.col(|ui| {
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.row() == Some(index) {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        crate::theme::palette(ui.ctx()).active_header,
                                    );
                                }
                                row_selector(
                                    ui,
                                    data,
                                    index,
                                    number,
                                    selection,
                                    (!collection_layout).then_some(pane),
                                );
                            });
                            for (position, (column, text)) in
                                data.columns.iter().zip(&data.rows[index]).enumerate()
                            {
                                row.col(|ui| {
                                    grid_cell(ui, false);
                                    let active = selection.selected(index, position);
                                    if ui.is_enabled() && selection.row() == Some(index) {
                                        let fill = if active {
                                            crate::theme::palette(ui.ctx()).active_cell
                                        } else {
                                            crate::theme::palette(ui.ctx()).active_row
                                        };
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, fill);
                                    }
                                    let response = cell(ui, text, &column.name, number, active);
                                    selection.observe(ui, index, position, &response);
                                    if !collection_layout
                                        && !selection.row_selected()
                                        && selection.selected(index, position)
                                    {
                                        focus::register(ui, pane, &response, true);
                                    }
                                    if ui.is_enabled() && selection.selected(index, position) {
                                        ui.painter().rect_stroke(
                                            ui.max_rect(),
                                            0.0,
                                            egui::Stroke::new(
                                                1.0,
                                                crate::theme::palette(ui.ctx()).accent,
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    if !selection.row_selected() {
                                        collect_contents_action(
                                            ui,
                                            &response,
                                            data,
                                            (index, position),
                                            actions,
                                        );
                                    }
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

fn column_header(
    ui: &mut Ui,
    data: &Data,
    index: usize,
    sort_revision: Option<u64>,
    actions: &mut Vec<Action>,
) -> Response {
    let column = &data.columns[index];
    let rect = ui.max_rect();
    let sort_width = sort_revision.map_or(0.0, |_| SORT_CONTROL_WIDTH);
    let caption = rect.shrink2(egui::vec2(sort_width, 0.0));
    let label = ui
        .put(
            caption,
            egui::Label::new(column.label())
                .halign(egui::Align::Center)
                .truncate(),
        )
        .on_hover_text(&column.name);
    if let Some(document_revision) = sort_revision {
        let control = egui::Rect::from_min_max(
            egui::pos2(rect.right() - sort_width, rect.top()),
            rect.right_bottom(),
        );
        let id =
            ui.make_persistent_id(("column_sort", document_revision, &data.table, &column.name));
        let response = ui.interact(control, id, Sense::click());
        let name = message(
            ui,
            "datasheet-sort-column",
            &[("column", column.name.clone())],
        );
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), name.clone()));
        let center = control.center();
        let half = SORT_CHEVRON_HALF_WIDTH;
        ui.painter()
            .with_clip_rect(control.intersect(ui.clip_rect()))
            .add(egui::Shape::line(
                vec![
                    center + egui::vec2(-half, -half * 0.5),
                    center + egui::vec2(0.0, half * 0.5),
                    center + egui::vec2(half, -half * 0.5),
                ],
                egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).ink),
            ));
        if response.enabled() {
            egui::Popup::menu(&response).show(|ui| {
                ui.small(tr(ui, "ribbon-sort-query-help"));
                for (descending, key) in [(false, "ribbon-ascending"), (true, "ribbon-descending")]
                {
                    if ui.button(tr(ui, key)).clicked() {
                        actions.push(Action::SortTable {
                            document_revision,
                            table: data.table.clone(),
                            column: column.name.clone(),
                            descending,
                        });
                        ui.close();
                    }
                }
            });
        } else {
            egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&response));
        }
        response.on_hover_text(tr(ui, "ribbon-sort-query-help"));
    }
    label
}

fn row_selector(
    ui: &mut Ui,
    data: &Data,
    row: usize,
    number: u64,
    selection: &mut Selection,
    document_pane: Option<Pane>,
) {
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    selection.observe_row(ui, data, row, &response);
    if let Some(pane) = document_pane
        && selection.row_selected()
        && selection.row() == Some(row)
    {
        focus::register(ui, pane, &response, true);
    }
    let label = message(
        ui,
        "datasheet-select-record",
        &[("number", number.to_string())],
    );
    response
        .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label.clone()));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::RowHeader);
        node.set_selected(selection.row_selected() && selection.row() == Some(row));
    });
    if response.has_focus() {
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
    response.context_menu(|ui| {
        if ui.button(tr(ui, "datasheet-copy-record")).clicked() {
            selection.copy(data, ui.ctx());
            ui.close();
        }
    });
    let key = if selection.row_selected() && selection.row() == Some(row) {
        "datasheet-record-selected-help"
    } else {
        "datasheet-select-record-help"
    };
    response.on_hover_text(message(ui, key, &[("number", number.to_string())]));
}

fn collect_contents_action(
    ui: &mut Ui,
    response: &Response,
    data: &Data,
    position: (usize, usize),
    actions: &mut Vec<Action>,
) {
    if !ui.is_enabled() {
        return;
    }
    let (row, column) = position;
    let contents = data.contents_target(row, column);
    let edit = data.edit_target(row, column);
    if contents.is_none() && edit.is_none() {
        return;
    }
    let mut view_contents = false;
    let mut edit_cell = false;
    response.context_menu(|ui| {
        if contents.is_some() && ui.button(tr(ui, "datasheet-view-contents")).clicked() {
            view_contents = true;
            ui.close();
        }
        if edit.is_some() && ui.button(tr(ui, "datasheet-edit-cell")).clicked() {
            edit_cell = true;
            ui.close();
        }
    });
    if contents.is_some() && response.has_focus() {
        view_contents |= ui.input_mut(|input| {
            let mut enter = false;
            input.events.retain(|event| {
                let plain_enter = matches!(event, egui::Event::Key {
                    key: egui::Key::Enter, pressed: true, modifiers, ..
                } if modifiers.is_none());
                enter |= plain_enter;
                !plain_enter
            });
            enter
        });
    }
    if view_contents && let Some(target) = contents {
        actions.push(Action::ViewContents { target });
    }
    if edit_cell && let Some(target) = edit {
        actions.push(Action::BeginEdit { target });
    }
}

fn grid_cell(ui: &Ui, header: bool) {
    let rect = ui.max_rect();
    if header {
        ui.painter()
            .rect_filled(rect, 0.0, crate::theme::palette(ui.ctx()).grid_header);
    }
    let stroke = egui::Stroke::new(0.5, crate::theme::palette(ui.ctx()).grid_line);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);
}

pub(crate) fn cell(
    ui: &mut Ui,
    value: &Cell,
    column: &str,
    row: u64,
    allow_copy: bool,
) -> Response {
    let original_text = value.text.as_str();
    let preview = preview::text(original_text);
    let text = preview.text.as_ref();
    let localizer = for_context(ui.ctx());
    let mut context = value.context(&localizer);
    if preview.shortened {
        let shortening = localizer.text("datasheet-value-preview-truncated");
        match &mut context {
            Some(context) => {
                context.push(' ');
                context.push_str(&shortening);
            }
            None => context = Some(shortening),
        }
    }
    let badge = value.badge(&localizer);
    let display = match badge.as_deref() {
        Some(badge) => Cow::Owned(format!("{text}  [{badge}]")),
        None => Cow::Borrowed(text),
    };
    let response = ui.allocate_response(ui.available_size(), Sense::click());
    let checkbox = value.checkbox_value();
    if let Some(checked) = checkbox {
        paint_boolean(ui, response.rect, checked, badge.as_deref());
    } else if matches!(value.kind, crate::CellKind::Scalar { is_null: true, .. }) {
        let mut marker = localizer.text("datasheet-null-value");
        if matches!(
            value.kind,
            crate::CellKind::Scalar {
                stored_calculation: true,
                ..
            }
        ) && let Some(badge) = badge
        {
            marker.push_str(&format!(" [{badge}]"));
        }
        let text = egui::RichText::new(marker)
            .italics()
            .color(crate::theme::palette(ui.ctx()).muted);
        let galley = egui::WidgetText::from(text).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            response.rect.width() - 2.0 * CELL_TEXT_PADDING,
            egui::FontSelection::Style(egui::TextStyle::Body),
        );
        ui.painter().galley(
            egui::pos2(
                response.rect.left() + CELL_TEXT_PADDING,
                response.rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            crate::theme::palette(ui.ctx()).muted,
        );
    } else {
        paint_cell_text(ui, response.rect, display.as_ref(), value.alignment);
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
    let accessible_label = match &context {
        Some(context) => message(
            ui,
            "datasheet-cell-details",
            &[
                ("column", column.to_owned()),
                ("row", row.to_string()),
                ("text", text.to_owned()),
                ("context", context.clone()),
            ],
        ),
        None => message(
            ui,
            "datasheet-cell-context",
            &[
                ("column", column.to_owned()),
                ("row", row.to_string()),
                ("text", text.to_owned()),
            ],
        ),
    };
    response.widget_info(|| {
        if let Some(checked) = checkbox {
            WidgetInfo::selected(
                WidgetType::Checkbox,
                ui.is_enabled(),
                checked,
                accessible_label.clone(),
            )
        } else {
            WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), accessible_label.clone())
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
            copy_text(ui.ctx(), original_text);
        }
        egui::Tooltip::for_widget(&response).show(|ui| {
            ui.label(text);
            if let Some(context) = &context {
                ui.small(context);
            }
            ui.small(tr(ui, "datasheet-copy-cell-help"));
        });
    }
    response.on_hover_ui(|ui| {
        ui.label(text);
        if let Some(context) = &context {
            ui.small(context);
        }
    })
}

fn paint_cell_text(ui: &Ui, rect: egui::Rect, text: &str, alignment: HorizontalAlignment) {
    let padding = CELL_TEXT_PADDING * ui.spacing().icon_width / BOOLEAN_BOX_SIZE;
    let width = (rect.width() - 2.0 * padding).max(0.0);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width,
        egui::TextStyle::Body,
    );
    let position = egui::pos2(
        match alignment {
            HorizontalAlignment::Left => rect.left() + padding,
            HorizontalAlignment::Right => rect.right() - padding - galley.size().x,
        },
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter()
        .galley(position, galley, ui.visuals().text_color());
}

fn paint_boolean(ui: &Ui, rect: egui::Rect, checked: bool, badge: Option<&str>) {
    let size = ui.spacing().icon_width.min(rect.height());
    let check = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(size));
    let color = crate::theme::palette(ui.ctx()).checkbox_accent;
    let fill = if checked {
        color
    } else {
        crate::theme::palette(ui.ctx()).surface
    };
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
            egui::Stroke::new(1.5, crate::theme::palette(ui.ctx()).surface),
        ));
    }
    if let Some(badge) = badge {
        let badge_rect =
            egui::Rect::from_min_max(egui::pos2(check.right() + 3.0, rect.top()), rect.max);
        paint_cell_text(
            ui,
            badge_rect,
            &format!("[{badge}]"),
            HorizontalAlignment::Left,
        );
    }
}

pub(crate) fn copy_text(ctx: &egui::Context, text: &str) {
    ctx.copy_text(text.to_owned());
}

#[cfg(test)]
mod tests;
