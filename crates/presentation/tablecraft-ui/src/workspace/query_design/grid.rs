//! Column-oriented QBE cells preserve local drafts and bounded literal input.
mod field;

use super::{MAX_DESIGN_COLUMNS, MAX_DESIGN_TEXT_BYTES, Session, totals};
use crate::localization::tr;
use crate::workspace::focus::{self, Pane};
use egui::{Align, Layout, ScrollArea, TextBuffer, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use tablecraft_builders::query::{DesignColumn, DesignExpression, DesignSort, DesignTotal};

const GRID_COLUMN_WIDTH: f32 = 175.0;
const GRID_LABEL_WIDTH: f32 = 85.0;
pub(super) const GRID_ROW_HEIGHT: f32 = 24.0;
const MAX_CRITERIA_ROWS: usize = tablecraft_builders::query::MAX_DESIGN_CRITERIA_ROWS;

#[derive(Clone, Copy, Debug, Hash)]
enum GridRow {
    Field,
    Table,
    Total,
    Sort,
    Show,
    Criterion(usize),
}

impl GridRow {
    fn at(index: usize, totals: bool) -> Self {
        match index {
            0 => Self::Field,
            1 => Self::Table,
            2 if totals => Self::Total,
            index => match index - usize::from(totals) {
                2 => Self::Sort,
                3 => Self::Show,
                index => Self::Criterion(index - 4),
            },
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Field => "qbe-field",
            Self::Table => "qbe-table",
            Self::Total => "qbe-total",
            Self::Sort => "qbe-sort",
            Self::Show => "qbe-show",
            Self::Criterion(0) => "qbe-criteria",
            Self::Criterion(_) => "qbe-or",
        }
    }
}

pub(super) fn design_grid(ui: &mut Ui, session: &mut Session) -> bool {
    let mut changed = false;
    let mut remove = None;
    let totals_enabled = session.draft.totals;
    let mut budget = InputBudget {
        remaining: MAX_DESIGN_TEXT_BYTES.saturating_sub(
            session
                .field_inputs
                .editable_bytes(&session.draft)
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
                body.rows(
                    GRID_ROW_HEIGHT,
                    4 + usize::from(totals_enabled) + criteria_rows,
                    |mut grid_row| {
                        let row = GridRow::at(grid_row.index(), totals_enabled);
                        grid_row.col(|ui| {
                            grid_cell(ui, true, |ui| {
                                ui.label(tr(ui, row.label()));
                            });
                        });
                        let sources = &session.draft.sources;
                        for (index, column) in session.draft.columns.iter_mut().enumerate() {
                            let where_only = totals_enabled && column.total == DesignTotal::Where;
                            grid_row.col(|ui| {
                                grid_cell(ui, false, |ui| {
                                    ui.push_id(("qbe_cell", index, row), |ui| match row {
                                        GridRow::Field => {
                                            let (_, edited, removed) = field::show(
                                                ui,
                                                field::Cell {
                                                    sources,
                                                    metadata: &session.metadata,
                                                    column,
                                                    index,
                                                },
                                                &mut session.field_inputs,
                                                &mut budget,
                                            );
                                            changed |= edited;
                                            if removed {
                                                remove = Some(index);
                                            }
                                        }
                                        GridRow::Table => {
                                            let table = match &column.expression {
                                                DesignExpression::Field(field) => sources
                                                    .iter()
                                                    .find(|source| source.id == field.source_id),
                                                DesignExpression::AllFields(Some(id)) => {
                                                    sources.iter().find(|source| source.id == *id)
                                                }
                                                _ => None,
                                            };
                                            ui.label(table.map_or("", |source| {
                                                source.alias.as_deref().unwrap_or(&source.table)
                                            }));
                                        }
                                        GridRow::Total => {
                                            changed |= totals::column(ui, column, index);
                                        }
                                        GridRow::Sort => {
                                            ui.add_enabled_ui(!where_only, |ui| {
                                                let response =
                                                    egui::ComboBox::from_id_salt("qbe_sort")
                                                        .selected_text(tr(
                                                            ui,
                                                            match column.sort {
                                                                None => "qbe-unsorted",
                                                                Some(DesignSort::Ascending) => {
                                                                    "qbe-ascending"
                                                                }
                                                                Some(DesignSort::Descending) => {
                                                                    "qbe-descending"
                                                                }
                                                            },
                                                        ))
                                                        .show_ui(ui, |ui| {
                                                            for (sort, key) in [
                                                                (None, "qbe-unsorted"),
                                                                (
                                                                    Some(DesignSort::Ascending),
                                                                    "qbe-ascending",
                                                                ),
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
                                            });
                                        }
                                        GridRow::Show => {
                                            let response = ui
                                                .with_layout(
                                                    Layout::centered_and_justified(
                                                        egui::Direction::LeftToRight,
                                                    ),
                                                    |ui| {
                                                        ui.add_enabled(
                                                            !where_only,
                                                            egui::Checkbox::without_text(
                                                                &mut column.show,
                                                            ),
                                                        )
                                                    },
                                                )
                                                .inner;
                                            let label =
                                                format!("{} {}", tr(ui, "qbe-show"), index + 1);
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
                                        GridRow::Criterion(criterion) => {
                                            let mut text = column
                                                .criteria
                                                .get(criterion)
                                                .cloned()
                                                .unwrap_or_default();
                                            let response = bounded_input(
                                                ui,
                                                &mut text,
                                                "qbe-criterion-input",
                                                column
                                                    .criteria
                                                    .get(criterion)
                                                    .map_or(0, String::len),
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
                    },
                );
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
                    total: DesignTotal::GroupBy,
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
        session.field_inputs.remove_column(index);
        session.draft.columns.remove(index);
        changed = true;
    }
    if budget.rejected {
        session.error = Some(tr(ui, "qbe-limit"));
    }
    changed
}

struct InputBudget {
    remaining: usize,
    rejected: bool,
}

fn bounded_input(
    ui: &mut Ui,
    text: &mut dyn TextBuffer,
    label_key: &str,
    old_cost: usize,
    budget: &mut InputBudget,
) -> egui::Response {
    let id = ui.make_persistent_id(label_key);
    let limit = budget.remaining.saturating_add(old_cost);
    budget.rejected |= crate::text_input::reject_oversized_events(ui, id, text.as_str(), limit);
    let label = tr(ui, label_key);
    let response = ui.add(
        TextEdit::singleline(&mut *text)
            .id(id)
            .desired_width(GRID_COLUMN_WIDTH)
            .frame(egui::Frame::NONE)
            .margin(egui::vec2(3.0, 1.0))
            .char_limit(limit.min(crate::text_input::MAX_EDITOR_SCALARS)),
    );
    if response.changed() {
        budget.remaining = limit.saturating_sub(text.as_str().len());
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
