//! Column-oriented QBE cells preserve local drafts and bounded literal input.
use super::{MAX_DESIGN_COLUMNS, MAX_DESIGN_TEXT_BYTES, Session, context_menu};
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use egui::{Align, Layout, ScrollArea, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignSort, DesignTableMetadata, QueryDesignDraft,
};

const GRID_COLUMN_WIDTH: f32 = 175.0;
const GRID_LABEL_WIDTH: f32 = 85.0;
pub(super) const GRID_ROW_HEIGHT: f32 = 24.0;
const MAX_CRITERIA_ROWS: usize = tablecraft_builders::query::MAX_DESIGN_CRITERIA_ROWS;

pub(super) fn design_grid(ui: &mut Ui, session: &mut Session) -> bool {
    let mut changed = false;
    let draft = session.draft.clone();
    let mut remove = None;
    let mut budget = InputBudget {
        remaining: MAX_DESIGN_TEXT_BYTES.saturating_sub(
            tablecraft_builders::query::draft_text_bytes(&session.draft)
                .unwrap_or(MAX_DESIGN_TEXT_BYTES),
        ),
        rejected: false,
    };
    let criteria_rows = session
        .draft
        .columns
        .iter()
        .map(|c| c.criteria.len())
        .max()
        .unwrap_or(2)
        .clamp(2, MAX_CRITERIA_ROWS);
    ScrollArea::both().id_salt("qbe_grid").show(ui, |ui| {
        let visible_columns = ((ui.available_width() - GRID_LABEL_WIDTH) / GRID_COLUMN_WIDTH)
            .ceil()
            .clamp(1.0, MAX_DESIGN_COLUMNS as f32) as usize;
        let column_count = session.draft.columns.len().max(visible_columns);
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            let mut table = TableBuilder::new(ui)
                .id_salt("qbe_cells")
                .vscroll(false)
                .striped(false)
                .auto_shrink([false, true])
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::exact(GRID_LABEL_WIDTH));
            for _ in 0..column_count {
                table = table.column(Column::exact(GRID_COLUMN_WIDTH));
            }
            table.body(|body| {
                body.rows(GRID_ROW_HEIGHT, 4 + criteria_rows, |mut grid_row| {
                    let row = grid_row.index();
                    grid_row.col(|ui| {
                        grid_cell(ui, true, |ui| {
                            ui.label(tr(
                                ui,
                                match row {
                                    0 => "qbe-field",
                                    1 => "qbe-table",
                                    2 => "qbe-sort",
                                    3 => "qbe-show",
                                    4 => "qbe-criteria",
                                    _ => "qbe-or",
                                },
                            ));
                        });
                    });
                    for (index, column) in session.draft.columns.iter_mut().enumerate() {
                        grid_row.col(|ui| {
                            grid_cell(ui, false, |ui| {
                                ui.push_id(("qbe_cell", index, row), |ui| match row {
                                    0 => {
                                        let original = field_text(&column.expression);
                                        let mut text = original.clone();
                                        let cost = match &column.expression {
                                            DesignExpression::Field(field) => field.column.len(),
                                            DesignExpression::Expression(text) => text.len(),
                                            DesignExpression::AllFields(_) => 0,
                                        };
                                        let previous_budget = budget.remaining;
                                        let response = bounded_input(
                                            ui,
                                            &mut text,
                                            "qbe-field-input",
                                            cost,
                                            &mut budget,
                                        );
                                        focus::register(
                                            ui,
                                            Pane::QueryGrid,
                                            &response,
                                            index == 0 || response.has_focus(),
                                        );
                                        if response.changed() && text != original {
                                            column.expression = edited_field(
                                                &draft,
                                                &session.metadata,
                                                &column.expression,
                                                text,
                                            );
                                            changed = true;
                                        } else {
                                            // A no-op replacement must not discard a bound
                                            // source or charge display-only wildcard text.
                                            budget.remaining = previous_budget;
                                        }
                                        context_menu::show(ui, &response, |ui, keyboard_open| {
                                            let mut alias =
                                                column.alias.clone().unwrap_or_default();
                                            ui.label(tr(ui, "qbe-alias"));
                                            let alias_input = bounded_input(
                                                ui,
                                                &mut alias,
                                                "qbe-alias-input",
                                                column.alias.as_ref().map_or(0, String::len),
                                                &mut budget,
                                            );
                                            if keyboard_open {
                                                alias_input.request_focus();
                                            }
                                            if alias_input.changed() {
                                                column.alias = (!alias.is_empty()).then_some(alias);
                                                changed = true;
                                            }
                                            if ui.button(tr(ui, "qbe-remove-column")).clicked() {
                                                remove = Some(index);
                                                ui.close();
                                            }
                                        });
                                    }
                                    1 => {
                                        let table = match &column.expression {
                                            DesignExpression::Field(field) => draft
                                                .sources
                                                .iter()
                                                .find(|source| source.id == field.source_id),
                                            DesignExpression::AllFields(Some(id)) => {
                                                draft.sources.iter().find(|source| source.id == *id)
                                            }
                                            _ => None,
                                        };
                                        ui.label(table.map_or("", |source| {
                                            source.alias.as_deref().unwrap_or(&source.table)
                                        }));
                                    }
                                    2 => {
                                        let response = egui::ComboBox::from_id_salt("qbe_sort")
                                            .selected_text(tr(
                                                ui,
                                                match column.sort {
                                                    None => "qbe-unsorted",
                                                    Some(DesignSort::Ascending) => "qbe-ascending",
                                                    Some(DesignSort::Descending) => {
                                                        "qbe-descending"
                                                    }
                                                },
                                            ))
                                            .show_ui(ui, |ui| {
                                                for (sort, key) in [
                                                    (None, "qbe-unsorted"),
                                                    (Some(DesignSort::Ascending), "qbe-ascending"),
                                                    (
                                                        Some(DesignSort::Descending),
                                                        "qbe-descending",
                                                    ),
                                                ] {
                                                    changed |= ui
                                                        .selectable_value(
                                                            &mut column.sort,
                                                            sort,
                                                            tr(ui, key),
                                                        )
                                                        .changed();
                                                }
                                            })
                                            .response;
                                        focus::register(
                                            ui,
                                            Pane::QueryGrid,
                                            &response,
                                            response.has_focus(),
                                        );
                                    }
                                    3 => {
                                        let response = ui
                                            .with_layout(
                                                Layout::centered_and_justified(
                                                    egui::Direction::LeftToRight,
                                                ),
                                                |ui| {
                                                    ui.add(egui::Checkbox::without_text(
                                                        &mut column.show,
                                                    ))
                                                },
                                            )
                                            .inner;
                                        let label = format!("{} {}", tr(ui, "qbe-show"), index + 1);
                                        ui.ctx().accesskit_node_builder(response.id, |node| {
                                            node.set_label(label)
                                        });
                                        changed |= response.changed();
                                        focus::register(
                                            ui,
                                            Pane::QueryGrid,
                                            &response,
                                            response.has_focus(),
                                        );
                                    }
                                    _ => {
                                        let criterion = row - 4;
                                        let mut text = column
                                            .criteria
                                            .get(criterion)
                                            .cloned()
                                            .unwrap_or_default();
                                        let response = bounded_input(
                                            ui,
                                            &mut text,
                                            "qbe-criterion-input",
                                            column.criteria.get(criterion).map_or(0, String::len),
                                            &mut budget,
                                        );
                                        focus::register(
                                            ui,
                                            Pane::QueryGrid,
                                            &response,
                                            response.has_focus(),
                                        );
                                        if response.changed() {
                                            if column.criteria.len() <= criterion {
                                                column
                                                    .criteria
                                                    .resize(criterion + 1, String::new());
                                            }
                                            column.criteria[criterion] = text;
                                            changed = true;
                                        }
                                    }
                                });
                            });
                        });
                    }
                    for _ in session.draft.columns.len()..column_count {
                        grid_row.col(|ui| grid_cell(ui, false, |_| {}));
                    }
                });
            });
        });
        ui.horizontal(|ui| {
            let add_expression = ui.add_enabled(
                session.draft.columns.len() < MAX_DESIGN_COLUMNS,
                egui::Button::new(tr(ui, "qbe-add-expression")),
            );
            focus::register(
                ui,
                Pane::QueryGrid,
                &add_expression,
                session.draft.columns.is_empty() || add_expression.has_focus(),
            );
            if add_expression.clicked() {
                session.draft.columns.push(DesignColumn {
                    expression: DesignExpression::Expression(String::new()),
                    alias: None,
                    show: true,
                    sort: None,
                    criteria: Vec::new(),
                });
                changed = true;
            }
            let add_criteria = ui.add_enabled(
                criteria_rows < MAX_CRITERIA_ROWS,
                egui::Button::new(tr(ui, "qbe-add-or")),
            );
            focus::register(ui, Pane::QueryGrid, &add_criteria, add_criteria.has_focus());
            if add_criteria.clicked() {
                for column in &mut session.draft.columns {
                    column.criteria.resize(criteria_rows + 1, String::new());
                }
                changed = true;
            }
        });
    });
    if let Some(index) = remove {
        session.draft.columns.remove(index);
        changed = true;
    }
    if budget.rejected {
        session.error = Some(tr(ui, "qbe-limit"));
    }
    changed
}

fn field_text(expression: &DesignExpression) -> String {
    match expression {
        DesignExpression::Field(field) => field.column.clone(),
        DesignExpression::AllFields(_) => "*".into(),
        DesignExpression::Expression(text) => text.clone(),
    }
}

fn edited_field(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    original: &DesignExpression,
    text: String,
) -> DesignExpression {
    let source_id = match original {
        DesignExpression::Field(field) => Some(field.source_id),
        DesignExpression::AllFields(source) => *source,
        DesignExpression::Expression(_) => None,
    };
    if let Some(source_id) = source_id {
        if text == "*" {
            return DesignExpression::AllFields(Some(source_id));
        }
        let source = draft.sources.iter().find(|source| source.id == source_id);
        let table =
            source.and_then(|source| metadata.iter().find(|table| table.table == source.table));
        if let Some(field) = table.and_then(|table| {
            table
                .fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(&text))
        }) {
            return DesignExpression::Field(DesignField {
                source_id,
                column: field.name.clone(),
            });
        }
        if table.is_some_and(|table| {
            table.fields.iter().any(|field| {
                field
                    .name
                    .get(..text.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&text))
            })
        }) {
            // Keep the Table row's source while a known local name is typed
            // one character at a time. Build still rejects incomplete names.
            return DesignExpression::Field(DesignField {
                source_id,
                column: text,
            });
        }
    }
    // Other text becomes an ordinary checked expression. The Table row clears;
    // ambiguous unqualified names must pass the builder's normal resolution.
    DesignExpression::Expression(text)
}

struct InputBudget {
    remaining: usize,
    rejected: bool,
}

fn bounded_input(
    ui: &mut Ui,
    text: &mut String,
    label_key: &str,
    old_cost: usize,
    budget: &mut InputBudget,
) -> egui::Response {
    let id = ui.make_persistent_id(label_key);
    let limit = budget.remaining.saturating_add(old_cost);
    budget.rejected |= crate::text_input::reject_oversized_events(ui, id, text, limit);
    let label = tr(ui, label_key);
    let response = ui.add(
        TextEdit::singleline(text)
            .id(id)
            .desired_width(GRID_COLUMN_WIDTH)
            .frame(egui::Frame::NONE)
            .margin(egui::vec2(3.0, 1.0))
            .char_limit(limit.min(crate::text_input::MAX_EDITOR_SCALARS)),
    );
    if response.changed() {
        budget.remaining = limit.saturating_sub(text.len());
    }
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label.as_str()));
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            0.0,
            egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).accent),
            egui::StrokeKind::Inside,
        );
        response.scroll_to_me(None);
    }
    response.on_hover_text(label)
}

/// A visual cell never gives blank QBE columns a draft identity or edit action.
fn grid_cell(ui: &mut Ui, label: bool, contents: impl FnOnce(&mut Ui)) {
    let rect = ui.max_rect();
    let colors = crate::theme::palette(ui.ctx());
    ui.painter().rect_filled(
        rect,
        0.0,
        if label {
            colors.grid_header
        } else {
            colors.surface
        },
    );
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, colors.grid_line),
        egui::StrokeKind::Inside,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(3.0, 0.0))),
        contents,
    );
}

#[cfg(test)]
mod tests;
