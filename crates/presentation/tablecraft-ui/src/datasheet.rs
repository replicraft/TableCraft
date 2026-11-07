mod accessibility;
mod add_column;
mod clipboard;
mod collection_columns;
mod empty_grid;
mod layout;
pub(crate) mod occurrence;
mod preview;
mod selection;
mod totals;
pub(crate) use selection::Selection;

use super::{Action, Cell, Data, HorizontalAlignment, InsertTarget, SortSpec, SortTarget};
use crate::localization::{for_context, message, tr};
use crate::workspace::focus::{self, Pane};
use crate::workspace::ribbon::Icon;
use egui::{Response, ScrollArea, Sense, Ui, WidgetInfo, WidgetType, accesskit::Role};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;
use std::ops::Range;

const RECORD_SELECTOR_WIDTH: f32 = 18.0;
const CELL_TEXT_PADDING: f32 = 3.0;
const BOOLEAN_BOX_SIZE: f32 = 14.0;
const HEADER_ROW_HEIGHT: f32 = 22.0;
const DATA_ROW_HEIGHT: f32 = 20.0;
const SORT_CONTROL_WIDTH: f32 = 18.0;
const SORT_CHEVRON_HALF_WIDTH: f32 = 3.0;

pub(crate) struct SortHeader {
    pub commands_enabled: bool,
    pub target: SortTarget,
    pub enabled: bool,
    pub sort: Option<SortSpec>,
}

pub(crate) enum SelectionOwner {
    Native,
    Total {
        target: crate::TotalsTarget,
        column: String,
    },
}

#[derive(Default)]
pub(crate) struct Output {
    pub selection: Option<SelectionOwner>,
    pub actions: Vec<Action>,
    pub totals_choices: Vec<crate::workspace::totals::Choice>,
}

#[derive(Default)]
pub(crate) struct Features<'a> {
    pub sort_header: Option<SortHeader>,
    pub insert_target: Option<InsertTarget>,
    pub totals: Option<crate::workspace::totals::Row<'a>>,
    pub occurrence: Option<occurrence::Occurrence>,
}

struct RowView<'a> {
    collection: bool,
    pane: Pane,
    sort_header: Option<SortHeader>,
    insert_target: Option<InsertTarget>,
    totals: Option<crate::workspace::totals::Row<'a>>,
    occurrence: Option<occurrence::Occurrence>,
}

pub(super) fn show(
    ui: &mut Ui,
    data: &Data,
    selection: &mut Selection,
    pane: Pane,
    features: Features<'_>,
) -> Output {
    let mut output = Output::default();
    if let Some(error) = selection.copy_error(ui.ctx()) {
        ui.colored_label(crate::theme::palette(ui.ctx()).accent, error);
    }
    render_rows(
        ui,
        data,
        |_| {},
        selection,
        RowView {
            collection: false,
            pane,
            sort_header: features.sort_header,
            insert_target: features.insert_target,
            totals: features.totals,
            occurrence: features.occurrence,
        },
        &mut output,
    );
    output
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
        selection,
        RowView {
            collection: true,
            pane: Pane::Document,
            sort_header: None,
            insert_target: None,
            totals: None,
            occurrence: None,
        },
        &mut Output::default(),
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
    let mut output = Output::default();
    let scroll = render_rows(
        ui,
        data,
        row_drawn,
        selection,
        RowView {
            collection: false,
            pane,
            sort_header: None,
            insert_target: None,
            totals: None,
            occurrence: None,
        },
        &mut output,
    );
    actions.extend(output.actions);
    scroll
}

fn render_rows(
    ui: &mut Ui,
    data: &Data,
    mut row_drawn: impl FnMut(usize),
    selection: &mut Selection,
    view: RowView<'_>,
    output: &mut Output,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    let Output {
        actions,
        totals_choices,
        selection: owner,
    } = output;
    if data.validate_row_ordinals().is_err() {
        return ScrollArea::both().show(ui, |ui| {
            ui.label(tr(ui, "datasheet-row-identity-invalid"));
        });
    }
    let collection_layout = view.collection;
    let add_column = !collection_layout && data.source_revision.is_some();
    let pane = view.pane;
    selection.prepare(ui, data);
    let scroll_to = selection.scroll_to_row();
    let mut page_layout = (!collection_layout).then(|| layout::PageLayout::load(ui, data));
    let scroll = ui
        .push_id((&data.table, data.offset), |ui| {
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
            let mut horizontal = ScrollArea::horizontal()
                .id_salt("datasheet_horizontal")
                .auto_shrink([false, false]);
            if let Some(layout) = &page_layout
                && layout.reset_page()
            {
                horizontal = horizontal.horizontal_scroll_offset(layout.horizontal_offset());
            }
            horizontal.show(ui, |ui| {
                let initial_widths =
                    collection_layout.then(|| collection_columns::initial_widths(ui, data));
                let insert = view.insert_target.as_ref().filter(|target| {
                    !data.has_more
                        && !data.columns.is_empty()
                        && data.source_revision == Some(target.document_revision)
                        && data.table == target.table
                });
                let visible_rows = data
                    .rows
                    .len()
                    .saturating_add(usize::from(insert.is_some()));
                let totals_height = if view.totals.is_some() {
                    DATA_ROW_HEIGHT * scale
                } else {
                    0.0
                };
                let body_height =
                    (ui.available_height() - HEADER_ROW_HEIGHT * scale - totals_height)
                        .max(DATA_ROW_HEIGHT * scale);
                let mut grid = accessibility::grid(ui, data);
                let mut header_row = accessibility::row(ui, &grid, 0, None);
                let mut column_rects = Vec::new();
                // Reserve background paint before cells and the empty-state
                // message; the rules must never cover actual record contents.
                let empty_grid = (!collection_layout).then(|| ui.painter().add(egui::Shape::Noop));
                let mut table = TableBuilder::new(ui).max_scroll_height(body_height);
                if page_layout
                    .as_ref()
                    .is_some_and(|layout| layout.reset_page())
                {
                    table.reset();
                }
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
                        let width = page_layout
                            .as_ref()
                            .and_then(|layout| layout.width(index))
                            .or_else(|| initial_widths.as_ref().map(|widths| widths[index]))
                            .unwrap_or(128.0);
                        Column::initial(width).at_least(60.0)
                    };
                    table = table.column(column);
                }
                if add_column {
                    table = table.column(Column::exact(add_column::WIDTH * scale).resizable(false));
                }
                let body = table
                    .header(HEADER_ROW_HEIGHT * scale, |mut header| {
                        header.col(|ui| {
                            column_rects.push(ui.max_rect());
                            grid_cell(ui, true);
                        });
                        for (index, column) in data.columns.iter().enumerate() {
                            header.col(|ui| {
                                column_rects.push(ui.max_rect());
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.column() == Some(index) {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        crate::theme::palette(ui.ctx()).active_header,
                                    );
                                }
                                accessibility::cell(
                                    ui,
                                    &mut header_row,
                                    index + 1,
                                    Role::ColumnHeader,
                                    |ui| {
                                        let label = column_header(
                                            ui,
                                            data,
                                            index,
                                            view.sort_header.as_ref(),
                                            actions,
                                        );
                                        let description = message(
                                            ui,
                                            "datasheet-field-name",
                                            &[("name", column.name.clone())],
                                        );
                                        ui.ctx().accesskit_node_builder(label.id, |node| {
                                            node.set_description(description);
                                        });
                                    },
                                );
                            });
                        }
                        if add_column {
                            header.col(add_column::header);
                        }
                    })
                    .body(|mut body| {
                        if let Some(layout) = &mut page_layout {
                            layout.observe_columns(body.widths(), data.columns.len());
                        }
                        if data.rows.is_empty() {
                            let mut bounds = body.max_rect();
                            if insert.is_some() {
                                bounds.min.y += DATA_ROW_HEIGHT * scale;
                            }
                            bounds.min.y += totals_height;
                            empty_body(body.ui_mut(), bounds, scale);
                        }
                        body.rows(DATA_ROW_HEIGHT * scale, visible_rows, |mut row| {
                            let index = row.index();
                            if index == data.rows.len() {
                                if let Some(target) = insert {
                                    row.col(|ui| {
                                        grid_cell(ui, true);
                                        ui.centered_and_justified(|ui| ui.label("*"));
                                    });
                                    for position in 0..data.columns.len() {
                                        row.col(|ui| {
                                            grid_cell(ui, false);
                                            if position == 0 {
                                                let button =
                                                    egui::Button::new(tr(ui, "insertion-title"))
                                                        .frame(false)
                                                        .truncate();
                                                if ui
                                                    .add_sized(ui.available_size(), button)
                                                    .clicked()
                                                {
                                                    actions.push(Action::BeginInsert {
                                                        target: target.clone(),
                                                    });
                                                }
                                            }
                                        });
                                    }
                                    if add_column {
                                        row.col(|ui| grid_cell(ui, false));
                                    }
                                }
                                return;
                            }
                            let mut semantic_row = None;
                            row_drawn(index);
                            let number = data
                                .display_ordinal(index)
                                .map_or(0, |ordinal| u64::from(ordinal) + 1);
                            row.col(|ui| {
                                grid_cell(ui, true);
                                if ui.is_enabled() && selection.row() == Some(index) {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        crate::theme::palette(ui.ctx()).active_header,
                                    );
                                }
                                let semantic_row = semantic_row.insert(accessibility::row(
                                    ui,
                                    &grid,
                                    index,
                                    Some(data.offset),
                                ));
                                accessibility::cell(
                                    ui,
                                    semantic_row,
                                    0,
                                    Role::GenericContainer,
                                    |ui| {
                                        let native_focus = row_selector(
                                            ui,
                                            data,
                                            index,
                                            number,
                                            selection,
                                            (!collection_layout).then_some(pane),
                                        );
                                        if native_focus {
                                            *owner = Some(SelectionOwner::Native);
                                        }
                                    },
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
                                    accessibility::cell(
                                        ui,
                                        semantic_row
                                            .as_mut()
                                            .expect("record selector precedes fields"),
                                        position + 1,
                                        Role::GridCell,
                                        |ui| {
                                            let occurrence =
                                                view.occurrence.as_ref().filter(|found| {
                                                    active
                                                        && found.row == index
                                                        && found.column == position
                                                });
                                            let response = cell_with_occurrence(
                                                ui,
                                                text,
                                                &column.name,
                                                number,
                                                active,
                                                occurrence,
                                            );
                                            if ui.is_enabled()
                                                && (response.has_focus()
                                                    || response.clicked()
                                                    || response.secondary_clicked())
                                            {
                                                *owner = Some(SelectionOwner::Native);
                                            }
                                            selection.observe(ui, index, position, &response);
                                            if !collection_layout
                                                && !selection.row_selected()
                                                && selection.selected(index, position)
                                            {
                                                focus::register(ui, pane, &response, true);
                                            }
                                            if ui.is_enabled()
                                                && selection.selected(index, position)
                                            {
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
                                                    !collection_layout,
                                                    actions,
                                                );
                                            }
                                        },
                                    );
                                });
                            }
                            if add_column {
                                row.col(|ui| grid_cell(ui, false));
                            }
                        });
                    });
                grid.expand_to_include_rect(body.inner_rect);
                for rect in &column_rects {
                    grid.expand_to_include_rect(*rect);
                }
                accessibility::bounds(&grid);
                if let Some(background) = empty_grid {
                    let content_bottom =
                        body.inner_rect.top() + body.content_size.y - body.state.offset.y;
                    ui.painter().set(
                        background,
                        empty_grid::shape(
                            &column_rects,
                            body.inner_rect,
                            content_bottom,
                            DATA_ROW_HEIGHT * scale,
                            crate::theme::palette(ui.ctx()).grid_line,
                        ),
                    );
                }
                if let Some(row) = &view.totals {
                    let header_bottom = column_rects
                        .first()
                        .map_or(body.inner_rect.top(), |rect| rect.bottom());
                    let content_bottom =
                        header_bottom + visible_rows as f32 * DATA_ROW_HEIGHT * scale;
                    let top = content_bottom.min(body.inner_rect.bottom());
                    totals::show(
                        ui,
                        row,
                        &column_rects,
                        top,
                        totals_height,
                        totals_choices,
                        owner,
                    );
                }
                // The inner vertical ScrollArea consumes both scroll targets,
                // including its disabled horizontal axis. Issue this one-shot
                // cell reveal only after that inner area has completed.
                if let Some(rect) = selection.take_reveal() {
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
            })
        })
        .inner;
    if let Some(layout) = page_layout {
        layout.finish(ui, scroll.state.offset.x);
    }
    scroll
}

fn empty_body(ui: &mut Ui, bounds: egui::Rect, scale: f32) {
    let visible = bounds.intersect(ui.clip_rect());
    let origin = visible.left_top()
        + egui::vec2(
            RECORD_SELECTOR_WIDTH * scale + CELL_TEXT_PADDING,
            CELL_TEXT_PADDING,
        );
    let message = tr(ui, "datasheet-no-rows");
    let color = crate::theme::palette(ui.ctx()).muted;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = (visible.right() - origin.x).max(0.0);
    let galley = ui.painter().layout(message.clone(), font, color, width);
    let message_rect = egui::Rect::from_min_size(origin, galley.size());
    ui.painter().galley(origin, galley, color);
    ui.interact(message_rect, ui.id().with("empty_body"), Sense::hover())
        .widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), message.clone()));
}

fn column_header(
    ui: &mut Ui,
    data: &Data,
    index: usize,
    sort_header: Option<&SortHeader>,
    actions: &mut Vec<Action>,
) -> Response {
    let column = &data.columns[index];
    let rect = ui.max_rect();
    let sort_width = sort_header.map_or(0.0, |_| SORT_CONTROL_WIDTH);
    let caption = rect.shrink2(egui::vec2(sort_width, 0.0));
    let label = ui
        .put(
            caption,
            egui::Label::new(column.label())
                .halign(egui::Align::Center)
                .truncate(),
        )
        .on_hover_text(&column.name);
    if let Some(header) = sort_header {
        let control = egui::Rect::from_min_max(
            egui::pos2(rect.right() - sort_width, rect.top()),
            rect.right_bottom(),
        );
        let id = ui.make_persistent_id((
            "column_sort",
            header.target.document_revision,
            header.target.filter_id,
            &data.table,
            &column.name,
        ));
        let response = ui
            .add_enabled_ui(header.commands_enabled, |ui| {
                ui.interact(control, id, Sense::click())
            })
            .inner;
        let name = message(
            ui,
            "datasheet-column-menu",
            &[("column", column.name.clone())],
        );
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), name.clone()));
        if let Some(sort) = &header.sort
            && sort.column == column.name
        {
            let marker = egui::Rect::from_center_size(
                egui::pos2(rect.left() + sort_width * 0.5, rect.center().y),
                egui::vec2(14.0, 14.0),
            );
            let icon = if sort.descending {
                Icon::Descending
            } else {
                Icon::Ascending
            };
            ui.put(marker, icon.image(ui, 14.0));
        }
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
                if ui.button(tr(ui, "filtering-field-title")).clicked() {
                    actions.push(Action::DescribeFieldFilter {
                        target: crate::FieldFilterTarget {
                            document_revision: header.target.document_revision,
                            table: header.target.table.clone(),
                            column: column.name.clone(),
                            filter_id: header.target.filter_id,
                            enabled: header.enabled,
                        },
                    });
                    ui.close();
                }
                ui.separator();
                ui.small(tr(ui, "filtering-sort-help"));
                for (descending, key) in [(false, "ribbon-ascending"), (true, "ribbon-descending")]
                {
                    if ui.button(tr(ui, key)).clicked() {
                        actions.push(Action::SortView {
                            target: header.target.clone(),
                            sort: Some(SortSpec {
                                column: column.name.clone(),
                                descending,
                            }),
                            enabled: header.enabled,
                        });
                        ui.close();
                    }
                }
            });
        } else {
            egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&response));
        }
        response.on_hover_text(tr(ui, "filtering-sort-help"));
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
) -> bool {
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
        node.set_role(Role::RowHeader);
        node.set_column_index(0);
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
    let native_focus = ui.is_enabled()
        && (response.has_focus() || response.clicked() || response.secondary_clicked());
    response.on_hover_text(message(ui, key, &[("number", number.to_string())]));
    native_focus
}

fn collect_contents_action(
    ui: &mut Ui,
    response: &Response,
    data: &Data,
    position: (usize, usize),
    allow_zoom: bool,
    actions: &mut Vec<Action>,
) {
    if !ui.is_enabled() {
        return;
    }
    let (row, column) = position;
    let contents = data.contents_target(row, column);
    let edit = data.edit_target(row, column);
    let value = data.rows.get(row).and_then(|values| values.get(column));
    let zoom = allow_zoom && value.is_some_and(crate::workspace::zoom::can_view);
    if contents.is_none() && edit.is_none() && !zoom {
        return;
    }
    let mut view_contents = false;
    let mut edit_cell = false;
    response.context_menu(|ui| {
        if zoom && ui.button(tr(ui, "zoom-title")).clicked() {
            if let Some(cell) = value {
                crate::workspace::zoom::request(ui.ctx(), cell, position);
            }
            ui.close();
        }
        if contents.is_some() && ui.button(tr(ui, "datasheet-view-contents")).clicked() {
            view_contents = true;
            ui.close();
        }
        if edit.is_some() && ui.button(tr(ui, "datasheet-edit-cell")).clicked() {
            edit_cell = true;
            ui.close();
        }
    });
    if zoom
        && response.has_focus()
        && !egui::Popup::is_any_open(ui.ctx())
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::F2))
        && let Some(cell) = value
    {
        crate::workspace::zoom::request(ui.ctx(), cell, position);
    }
    if response.has_focus()
        && !egui::Popup::is_any_open(ui.ctx())
        && let Some(target) = edit.as_ref()
    {
        let pasted = ui.input_mut(|input| {
            let index = input
                .events
                .iter()
                .position(|event| matches!(event, egui::Event::Paste(_)))?;
            match input.events.remove(index) {
                egui::Event::Paste(text) => Some(text),
                _ => None,
            }
        });
        if let Some(text) = pasted {
            actions.push(Action::PasteCell {
                target: target.clone(),
                text,
            });
        }
    }
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
    cell_with_occurrence(ui, value, column, row, allow_copy, None)
}

fn cell_with_occurrence(
    ui: &mut Ui,
    value: &Cell,
    column: &str,
    row: u64,
    allow_copy: bool,
    occurrence: Option<&occurrence::Occurrence>,
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
        let range = occurrence.and_then(|found| found.visible_range(value, &preview));
        paint_cell_text_with_occurrence(
            ui,
            response.rect,
            display.as_ref(),
            value.alignment,
            range,
        );
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
    }
    response.on_hover_ui(|ui| {
        ui.label(text);
        if let Some(context) = &context {
            ui.small(context);
        }
        if allow_copy {
            ui.small(tr(ui, "datasheet-copy-cell-help"));
        }
    })
}

fn paint_cell_text(ui: &Ui, rect: egui::Rect, text: &str, alignment: HorizontalAlignment) {
    paint_cell_text_with_occurrence(ui, rect, text, alignment, None);
}

fn paint_cell_text_with_occurrence(
    ui: &Ui,
    rect: egui::Rect,
    text: &str,
    alignment: HorizontalAlignment,
    occurrence: Option<Range<usize>>,
) {
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
    if let Some(range) = occurrence {
        occurrence::paint(ui, rect, position, &galley, range);
    }
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
