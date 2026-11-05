//! A derived view row shares actual table columns, never native record identity.
use crate::HorizontalAlignment;
use crate::localization::{message, tr};
use crate::workspace::totals::{Choice, Row};
use egui::{ComboBox, Rect, RichText, Sense, Ui, UiBuilder};

pub(super) fn show(
    ui: &mut Ui,
    row: &Row<'_>,
    columns: &[Rect],
    top: f32,
    height: f32,
    choices: &mut Vec<Choice>,
    owner: &mut Option<super::SelectionOwner>,
) {
    let Some(first) = columns.first() else {
        return;
    };
    let Some(last) = columns.last() else {
        return;
    };
    let bounds = Rect::from_min_max(
        egui::pos2(first.left(), top),
        egui::pos2(last.right(), top + height),
    );
    ui.allocate_rect(bounds, Sense::hover());
    for (index, column) in columns.iter().enumerate() {
        let rect = Rect::from_min_max(
            egui::pos2(column.left(), top),
            egui::pos2(column.right(), top + height),
        );
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            super::grid_cell(ui, true);
            if index == 0 {
                return;
            }
            let Some(fields) = row.fields else {
                if index == 1 {
                    ui.label(tr(ui, "totals-caption"));
                }
                return;
            };
            let Some(field) = fields.get(index - 1) else {
                return;
            };
            total_cell(ui, row, field, index == 1, choices, owner);
        });
    }
}

fn total_cell(
    ui: &mut Ui,
    row: &Row<'_>,
    field: &crate::TotalsField,
    first: bool,
    choices: &mut Vec<Choice>,
    owner: &mut Option<super::SelectionOwner>,
) {
    let spec = row
        .specs
        .iter()
        .find(|spec| spec.column == field.column.name);
    let value = row
        .result
        .filter(|result| result.target == *row.target && result.specs == row.specs);
    let cell = value.and_then(|result| {
        let index = result
            .specs
            .iter()
            .position(|spec| spec.column == field.column.name)?;
        result.cells.get(index)
    });
    let text = if spec.is_none() {
        if first {
            tr(ui, "totals-caption")
        } else {
            String::new()
        }
    } else if row.pending {
        "…".into()
    } else if row.error {
        "!".into()
    } else {
        cell.map_or_else(String::new, |cell| cell.text.clone())
    };
    let label = message(
        ui,
        "totals-column",
        &[("column", field.column.name.clone())],
    );
    let enabled = row.enabled && !row.pending && (!field.aggregates.is_empty() || spec.is_some());
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            let combo = ComboBox::from_id_salt(("total", &field.column.name))
                .selected_text("")
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(spec.is_none(), tr(ui, "totals-none"))
                        .clicked()
                    {
                        choices.push(Choice {
                            target: row.target.clone(),
                            column: field.column.name.clone(),
                            aggregate: None,
                        });
                        ui.close();
                    }
                    for aggregate in &field.aggregates {
                        let selected = spec.is_some_and(|spec| spec.aggregate == *aggregate);
                        if ui
                            .selectable_label(selected, tr(ui, aggregate.key()))
                            .clicked()
                        {
                            choices.push(Choice {
                                target: row.target.clone(),
                                column: field.column.name.clone(),
                                aggregate: Some(*aggregate),
                            });
                            ui.close();
                        }
                    }
                });
            let popup_open = combo.inner.is_some();
            (combo.response, popup_open)
        })
        .inner;
    let (response, popup_open) = response;
    if response.enabled() && (response.has_focus() || response.clicked() || popup_open) {
        *owner = Some(super::SelectionOwner::Total {
            target: row.target.clone(),
            column: field.column.name.clone(),
        });
    }
    if ui.is_enabled() && row.selected_column == Some(field.column.name.as_str()) {
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).accent),
            egui::StrokeKind::Inside,
        );
    }
    let alignment = cell.map_or(HorizontalAlignment::Left, |cell| cell.alignment);
    paint_value(ui, response.rect, &text, alignment);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
        node.set_value(text.clone());
    });
    if response.has_focus() && cell.is_some() && !row.pending && !row.error {
        let mut copy = false;
        ui.input_mut(|input| {
            input.events.retain(|event| {
                if matches!(event, egui::Event::Copy) {
                    copy = true;
                    false
                } else {
                    true
                }
            });
        });
        if copy {
            ui.ctx().copy_text(text.clone());
        }
    }
    response.on_hover_text(text);
}

fn paint_value(ui: &Ui, rect: Rect, text: &str, alignment: HorizontalAlignment) {
    let padding = ui.spacing().button_padding.x;
    let right = rect.right() - ui.spacing().icon_width - padding;
    let available = (right - rect.left() - padding).max(0.0);
    let galley = egui::WidgetText::from(RichText::new(text).strong()).into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        available,
        egui::TextStyle::Body,
    );
    let left = match alignment {
        HorizontalAlignment::Left => rect.left() + padding,
        HorizontalAlignment::Right => right - galley.size().x,
    };
    let position = egui::pos2(left, rect.center().y - galley.size().y * 0.5);
    ui.painter()
        .galley(position, galley, ui.visuals().text_color());
}

#[cfg(test)]
mod tests;
