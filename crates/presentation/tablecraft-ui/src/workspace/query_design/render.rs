//! The source area and QBE rows follow the retained native Design captures.
use super::click;
use super::sources::source_area;
use super::{MAX_DESIGN_COLUMNS, MAX_DESIGN_TEXT_BYTES, Mode, Session};
use crate::localization::tr;
use crate::workspace::{Action, State, diagnostics};
use egui::{Frame, Grid, ScrollArea, TextEdit, Ui};
use tablecraft_builders::query::{DesignColumn, DesignExpression, DesignSort, expression_text};

const GRID_COLUMN_WIDTH: f32 = 175.0;
const TABLE_LIST_HEIGHT: f32 = 240.0;
const MAX_CRITERIA_ROWS: usize = tablecraft_builders::query::MAX_DESIGN_CRITERIA_ROWS;

impl State {
    pub(in crate::workspace) fn query_design_editor(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        self.query_design_views(ui, actions);
        if !self.query_design_visible() {
            return;
        }
        let ready = self.query_design_ready();
        let mut session = self
            .query_design
            .session
            .take()
            .expect("visible Design session");
        let mut run_clicked = false;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(ready, egui::Button::new(tr(ui, "qbe-add-tables")))
                .clicked()
            {
                session.add_tables = true;
            }
            run_clicked = ui
                .add_enabled(ready, egui::Button::new(tr(ui, "query-run")))
                .clicked();
            ui.small(tr(ui, "qbe-help"));
        });
        ui.add_enabled_ui(ready, |ui| {
            let mut changed = source_area(ui, &mut session);
            ui.separator();
            changed |= design_grid(ui, &mut session);
            if changed {
                session.revision = session.revision.wrapping_add(1);
            }
        });
        self.query_design.session = Some(session);
        if run_clicked && let Some(action) = self.query_design_run_action() {
            actions.push(action);
        }
    }

    pub(in crate::workspace) fn query_design_report(&self, ui: &mut Ui) {
        if self.query_design_request().is_none() {
            return;
        }
        if let Some(error) = self
            .query_design
            .session
            .as_ref()
            .and_then(|session| session.error.as_ref())
        {
            diagnostics::show(ui, ui.make_persistent_id("query_design_error"), error);
        }
    }

    pub(in crate::workspace) fn query_design_views(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        if self.query_design_request().is_none() {
            return;
        }
        let ready = self.query_design_ready();
        ui.horizontal(|ui| {
            let sql = !self.query_design_visible() && !self.query_design_results_visible();
            if ui
                .add_enabled(
                    ready,
                    egui::Button::selectable(sql, tr(ui, "query-sql-view")),
                )
                .clicked()
                && let Some(action) = self.show_query_design_sql()
            {
                actions.push(action);
            }
            if ui
                .add_enabled(
                    ready,
                    egui::Button::selectable(
                        self.query_design_visible(),
                        tr(ui, "query-design-view"),
                    ),
                )
                .clicked()
                && let Some(action) = self.show_query_design()
            {
                actions.push(action);
            }
            if ui
                .add_enabled(
                    ready && self.query_data.is_some(),
                    egui::Button::selectable(
                        self.query_design_results_visible(),
                        tr(ui, "ribbon-datasheet"),
                    ),
                )
                .clicked()
            {
                self.query_design
                    .session
                    .get_or_insert_with(Default::default)
                    .mode = Mode::Results;
            }
        });
    }

    pub(in crate::workspace) fn query_design_dialog(
        &mut self,
        context: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        if !self.query_design_dialog_open() {
            return;
        }
        let request = self.query_design_request().expect("source-bound dialog");
        let pending = self.query_design_request_id();
        let tables: Vec<String> = self
            .database
            .as_ref()
            .expect("current source")
            .tables
            .iter()
            .filter(|table| table.local && !table.system)
            .map(|table| table.name.clone())
            .collect();
        let mut session = self
            .query_design
            .session
            .take()
            .expect("open Add Tables dialog");
        if session
            .selected_table
            .as_ref()
            .is_none_or(|selected| !tables.contains(selected))
        {
            session.selected_table = tables.first().cloned();
        }
        let mut close = false;
        let mut row_clicked = false;
        egui::Modal::new(egui::Id::new(("query_design_tables", request.tab.id))).show(
            context,
            |ui| {
                ui.set_width(350.0_f32.min(context.content_rect().width() - 48.0));
                ui.heading(tr(ui, "qbe-add-tables"));
                ui.label(tr(ui, "qbe-tables"));
                Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ScrollArea::vertical()
                        .max_height(TABLE_LIST_HEIGHT)
                        .min_scrolled_height(TABLE_LIST_HEIGHT)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for table in &tables {
                                let selected = session.selected_table.as_ref() == Some(table);
                                let response = ui
                                    .push_id(("qbe_table", table), |ui| {
                                        ui.add_enabled(
                                            pending.is_none(),
                                            egui::Button::selectable(selected, table),
                                        )
                                    })
                                    .inner;
                                if response.clicked() {
                                    row_clicked = true;
                                    session.selected_table = Some(table.clone());
                                    if click::activate(ui, &response, &mut session.click_pair) {
                                        actions.push(Action::DescribeQueryDesign {
                                            request: request.clone(),
                                            table: table.clone(),
                                        });
                                    }
                                }
                            }
                        });
                });
                if let Some(error) = &session.error {
                    diagnostics::show(ui, ui.make_persistent_id("query_design_table_error"), error);
                }
                let labels = [tr(ui, "qbe-add"), tr(ui, "qbe-close")];
                let font = egui::TextStyle::Button.resolve(ui.style());
                let width = labels
                    .iter()
                    .map(|label| {
                        ui.painter()
                            .layout_no_wrap(label.clone(), font.clone(), ui.visuals().text_color())
                            .size()
                            .x
                            + 2.0 * ui.spacing().button_padding.x
                    })
                    .sum::<f32>()
                    + ui.spacing().item_spacing.x;
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - width).max(0.0));
                    if ui
                        .add_enabled(
                            pending.is_none() && session.selected_table.is_some(),
                            egui::Button::new(tr(ui, "qbe-add")),
                        )
                        .clicked()
                    {
                        session.click_pair = None;
                        actions.push(Action::DescribeQueryDesign {
                            request: request.clone(),
                            table: session.selected_table.clone().expect("enabled selection"),
                        });
                    }
                    close |= ui.button(tr(ui, "qbe-close")).clicked();
                });
                close |= ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            },
        );
        if !row_clicked && context.input(|input| input.pointer.any_released()) {
            session.click_pair = None;
        }
        if close {
            session.click_pair = None;
            session.add_tables = false;
            if let Some(request_id) = pending {
                actions.push(Action::CancelQueryDesign { request_id });
            }
        }
        self.query_design.session = Some(session);
    }
}

fn design_grid(ui: &mut Ui, session: &mut Session) -> bool {
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
        Grid::new("qbe_cells")
            .striped(true)
            .min_col_width(90.0)
            .show(ui, |ui| {
                for row in 0..4 + criteria_rows {
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
                    for (index, column) in session.draft.columns.iter_mut().enumerate() {
                        ui.push_id(("qbe_cell", index, row), |ui| {
                            ui.set_min_width(GRID_COLUMN_WIDTH);
                            match row {
                                0 => {
                                    let mut text = expression_text(&draft, &column.expression)
                                        .unwrap_or_default();
                                    let cost = match &column.expression {
                                        DesignExpression::Field(field) => field.column.len(),
                                        DesignExpression::Expression(text) => text.len(),
                                        DesignExpression::AllFields(_) => 0,
                                    };
                                    let response = bounded_input(
                                        ui,
                                        &mut text,
                                        "qbe-field-input",
                                        cost,
                                        &mut budget,
                                    );
                                    if response.changed() {
                                        column.expression = DesignExpression::Expression(text);
                                        changed = true;
                                    }
                                    response.context_menu(|ui| {
                                        let mut alias = column.alias.clone().unwrap_or_default();
                                        ui.label(tr(ui, "qbe-alias"));
                                        if bounded_input(
                                            ui,
                                            &mut alias,
                                            "qbe-alias-input",
                                            column.alias.as_ref().map_or(0, String::len),
                                            &mut budget,
                                        )
                                        .changed()
                                        {
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
                                    egui::ComboBox::from_id_salt("qbe_sort")
                                        .selected_text(tr(
                                            ui,
                                            match column.sort {
                                                None => "qbe-unsorted",
                                                Some(DesignSort::Ascending) => "qbe-ascending",
                                                Some(DesignSort::Descending) => "qbe-descending",
                                            },
                                        ))
                                        .show_ui(ui, |ui| {
                                            for (sort, key) in [
                                                (None, "qbe-unsorted"),
                                                (Some(DesignSort::Ascending), "qbe-ascending"),
                                                (Some(DesignSort::Descending), "qbe-descending"),
                                            ] {
                                                changed |= ui
                                                    .selectable_value(
                                                        &mut column.sort,
                                                        sort,
                                                        tr(ui, key),
                                                    )
                                                    .changed();
                                            }
                                        });
                                }
                                3 => {
                                    let response = ui.checkbox(&mut column.show, "");
                                    let label = format!("{} {}", tr(ui, "qbe-show"), index + 1);
                                    ui.ctx().accesskit_node_builder(response.id, |node| {
                                        node.set_label(label)
                                    });
                                    changed |= response.changed();
                                }
                                _ => {
                                    let criterion = row - 4;
                                    let mut text =
                                        column.criteria.get(criterion).cloned().unwrap_or_default();
                                    if bounded_input(
                                        ui,
                                        &mut text,
                                        "qbe-criterion-input",
                                        column.criteria.get(criterion).map_or(0, String::len),
                                        &mut budget,
                                    )
                                    .changed()
                                    {
                                        if column.criteria.len() <= criterion {
                                            column.criteria.resize(criterion + 1, String::new());
                                        }
                                        column.criteria[criterion] = text;
                                        changed = true;
                                    }
                                }
                            }
                        });
                    }
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    session.draft.columns.len() < MAX_DESIGN_COLUMNS,
                    egui::Button::new(tr(ui, "qbe-add-expression")),
                )
                .clicked()
            {
                session.draft.columns.push(DesignColumn {
                    expression: DesignExpression::Expression(String::new()),
                    alias: None,
                    show: true,
                    sort: None,
                    criteria: Vec::new(),
                });
                changed = true;
            }
            if ui
                .add_enabled(
                    criteria_rows < MAX_CRITERIA_ROWS,
                    egui::Button::new(tr(ui, "qbe-add-or")),
                )
                .clicked()
            {
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
            .hint_text(&label)
            .char_limit(limit.min(crate::text_input::MAX_EDITOR_SCALARS)),
    );
    if response.changed() {
        budget.remaining = limit.saturating_sub(text.len());
    }
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label.as_str()));
    response.on_hover_text(label)
}
