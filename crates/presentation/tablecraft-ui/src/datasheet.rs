use super::{Action, Cell, Data};
use crate::theme::{GRID_HEADER, GRID_LINE};
use egui::{Response, RichText, ScrollArea, Sense, Ui, WidgetInfo, WidgetType};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;

pub(super) fn show(ui: &mut Ui, data: &Data) -> Vec<Action> {
    let mut actions = Vec::new();
    show_rows(ui, data, |_| {}, &mut actions);
    actions
}

fn show_rows(
    ui: &mut Ui,
    data: &Data,
    mut row_drawn: impl FnMut(usize),
    actions: &mut Vec<Action>,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    ui.push_id((&data.table, data.offset), |ui| {
        // Reserve scrollbar space below the viewport, including one-row pages.
        // The style change is scoped to this datasheet and its nested table.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        ScrollArea::horizontal()
            .id_salt("datasheet_horizontal")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                TableBuilder::new(ui)
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
                        for name in &data.columns {
                            header.col(|ui| {
                                grid_cell(ui, true);
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
                                ui.weak(number.to_string());
                            });
                            for (position, (column, text)) in
                                data.columns.iter().zip(&data.rows[index]).enumerate()
                            {
                                row.col(|ui| {
                                    grid_cell(ui, false);
                                    let response = cell(ui, text, column, number);
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

fn cell(ui: &mut Ui, value: &Cell, column: &str, row: u64) -> Response {
    let text = value.text.as_str();
    let context = value.context();
    let display = match value.badge() {
        Some(badge) => Cow::Owned(format!("{text}  [{badge}]")),
        None => Cow::Borrowed(text),
    };
    let response = ui.add(
        egui::Label::new(display.as_ref())
            .selectable(true)
            .sense(Sense::focusable_noninteractive())
            .truncate(),
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        WidgetInfo::labeled(
            WidgetType::Label,
            ui.is_enabled(),
            match &context {
                Some(context) => format!("{column}, row {row}: {text}; {context}"),
                None => format!("{column}, row {row}: {text}"),
            },
        )
    });

    if response.has_focus() && ui.is_enabled() {
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
            ui.ctx().copy_text(text.to_owned());
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

#[cfg(test)]
mod tests;
