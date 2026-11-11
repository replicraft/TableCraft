//! Read-only Table Design uses host-supplied facts without interpreting storage.
mod grid;
mod properties;
mod type_labels;
pub use properties::{FieldProperty, FieldPropertyValue};

use super::{DocumentView, State};
use crate::workspace::focus::{self, Pane};
use egui::{Id, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;

const MAX_PANE_ENTRY_FRAMES: u64 = 8;
const FIELD_ROW_HEIGHT: f32 = 19.0;
const SELECTOR_WIDTH: f32 = 18.0;
const FIELD_NAME_WIDTH: f32 = 192.0;
const DATA_TYPE_WIDTH: f32 = 160.0;
const MIN_GRID_HEIGHT: f32 = 90.0;
const MIN_PROPERTIES_HEIGHT: f32 = 140.0;
const SPLITTER_HEIGHT: f32 = 6.0;

/// Captured declaration metadata, separate from table pages and query results.
pub struct Schema {
    pub table: String,
    pub declared_rows: u32,
    pub fields: Vec<Field>,
}

/// Native declaration facts and the backend's current reader classification.
pub struct Field {
    pub name: String,
    pub number: usize,
    pub native_type: u8,
    pub storage_bytes: usize,
    pub fixed: bool,
    pub flags: u8,
    pub extended_flags: u8,
    /// Exact reader-carrier identifier, retained verbatim in native diagnostics.
    pub scalar_type: Option<String>,
    /// Optional host-supplied Access presentation name, never inferred from flags.
    /// Known UI names localize; unknown labels remain literal.
    pub display_type: Option<String>,
    /// Host-authored display summary, distinct from raw stored property text.
    /// Numeric and unknown summaries remain literal; known UI labels localize.
    pub field_size: Option<String>,
    pub reader_nullable: Option<bool>,
    pub decimal_parameters: Option<(u8, u8)>,
    pub complex_type_id: Option<u32>,
    pub properties: Vec<FieldProperty>,
}

/// A logical entry request, never a cached widget response or identifier.
pub(super) struct PaneEntry {
    publication: u64,
    document_revision: Option<u64>,
    table: String,
    position: usize,
    field_number: usize,
    field_name: String,
    started_frame: u64,
    last_frame: u64,
    // Comparison only; this identifier is never restored as a focus target.
    focus_witness: Option<Id>,
}

impl State {
    pub(super) fn request_design_pane_entry(&mut self, context: &egui::Context) {
        let Some(schema) = &self.schema else {
            return;
        };
        let Some(field) = schema.fields.get(self.selected_field) else {
            return;
        };
        let frame = context.cumulative_frame_nr();
        self.design_focus_pending = Some(PaneEntry {
            publication: self.focus_publication,
            document_revision: self
                .database
                .as_ref()
                .map(|database| database.document_revision),
            // NO_CLONE: Retain the exact prior source, selected column, scope, pattern or
            // publication binding across mutation/publication. These owners reject
            // stale work or preserve the other input; they cannot borrow the mutated
            // State.
            table: schema.table.clone(),
            position: self.selected_field,
            field_number: field.number,
            // NO_CLONE: Retain the exact prior source, selected column, scope, pattern or
            // publication binding across mutation/publication. These owners reject
            // stale work or preserve the other input; they cannot borrow the mutated
            // State.
            field_name: field.name.clone(),
            started_frame: frame,
            last_frame: frame,
            focus_witness: context.memory(|memory| memory.focused()),
        });
    }

    fn take_design_pane_entry(&mut self, context: &egui::Context) -> Option<PaneEntry> {
        let entry = self.design_focus_pending.take()?;
        let schema = self.schema.as_ref()?;
        let field = schema.fields.get(self.selected_field)?;
        let frame = context.cumulative_frame_nr();
        let current = context.memory(|memory| memory.focused()) == entry.focus_witness
            && self.view == DocumentView::Design
            && entry.publication == self.focus_publication
            && entry.document_revision
                == self
                    .database
                    .as_ref()
                    .map(|database| database.document_revision)
            && entry.table == schema.table
            && entry.position == self.selected_field
            && entry.field_number == field.number
            && entry.field_name == field.name
            && (frame == entry.last_frame || frame == entry.last_frame.saturating_add(1))
            && frame.saturating_sub(entry.started_frame) <= MAX_PANE_ENTRY_FRAMES;
        current.then_some(entry)
    }

    pub(super) fn design(&mut self, ui: &mut Ui) {
        let entry = self.take_design_pane_entry(ui.ctx());
        let enter_grid = entry.is_some();
        let mut entered_grid = false;
        // NO_CLONE: Share the immutable localization bundle across mutable widget work;
        // Localizer clones its Arc owner, not the catalogs.
        let localizer = self.localizer.clone();
        let rename_tab = self.active_object();
        let rename_enabled = rename_tab
            .as_ref()
            .is_some_and(|tab| self.field_rename_table_entry_ready(tab));
        let drop_enabled = rename_tab
            .as_ref()
            .is_some_and(|tab| self.field_drop_table_entry_ready(tab));
        let pending_drop = self.field_drop_pending_target();
        let rename_editor = self.field_rename_editor_presentation();
        let mut saved_field_event = None;
        let addition = self.field_addition_render_context();
        let addition_kind = self
            .field_addition_declaration()
            .map(|declaration| declaration.kind());
        // The pending declaration owns the property pane without becoming a
        // stored field. Keep the prior native selection for its eventual return.
        let addition_pending = addition
            .as_ref()
            .is_some_and(|(_, presentation)| presentation.name.is_some());
        let addition_caption = addition.as_ref().is_some_and(|(_, presentation)| {
            presentation.name.is_some() || (presentation.enabled && presentation.choices.any())
        });
        let mut addition_event = None;
        let Some(schema) = &self.schema else {
            ui.weak(crate::localization::tr(ui, "design-choose"));
            return;
        };
        ui.small(crate::localization::tr(
            ui,
            if addition_caption {
                "design-add-fields-save"
            } else if self.field_drop_available {
                "field-drop-design-help"
            } else if self.field_rename_available {
                "field-rename-design-help"
            } else {
                "design-read-only"
            },
        ))
        .on_hover_text(crate::localization::tr(ui, "design-inert-help"));
        if schema.fields.is_empty() {
            ui.weak(crate::localization::tr(ui, "design-no-fields"));
            return;
        }

        let focus_scope = ui.id();
        let document_revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let scroll_to = navigate_fields(
            ui,
            focus_scope,
            document_revision,
            schema,
            &mut self.selected_field,
        )
        .or_else(|| enter_grid.then_some(self.selected_field));

        let grid_height = pane_height(ui);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), grid_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let mut table = TableBuilder::new(ui)
                    .animate_scrolling(false)
                    .max_scroll_height(grid_height - FIELD_ROW_HEIGHT);
                if let Some(position) = scroll_to {
                    table = table.scroll_to_row(position, None);
                }
                table
                    .striped(false)
                    .resizable(true)
                    .column(Column::exact(SELECTOR_WIDTH).resizable(false))
                    .column(Column::initial(FIELD_NAME_WIDTH).at_least(100.0))
                    .column(Column::initial(DATA_TYPE_WIDTH).at_least(90.0))
                    .column(Column::remainder().at_least(70.0))
                    .header(FIELD_ROW_HEIGHT, |mut header| {
                        header.col(|ui| grid::selector(ui, false));
                        for (position, title) in [
                            localizer.text("design-field-name"),
                            localizer.text("design-data-type"),
                            localizer.text("design-description-optional"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            header.col(|ui| {
                                grid::cell(
                                    ui,
                                    &title,
                                    grid::CellStyle::Header {
                                        active: position == 0,
                                    },
                                );
                            });
                        }
                    })
                    .body(|body| {
                        let visible_slots =
                            ((grid_height - FIELD_ROW_HEIGHT) / FIELD_ROW_HEIGHT).ceil() as usize;
                        let row_count = schema.fields.len().saturating_add(1).max(visible_slots);
                        body.rows(FIELD_ROW_HEIGHT, row_count, |mut row| {
                            let position = row.index();
                            let Some(field) = schema.fields.get(position) else {
                                row.col(|ui| {
                                    grid::selector(
                                        ui,
                                        addition_pending && position == schema.fields.len(),
                                    )
                                });
                                if position == schema.fields.len()
                                    && let Some((_, presentation)) = &addition
                                {
                                    row.col(|ui| {
                                        addition_event =
                                            super::field_addition::design::name(ui, presentation);
                                    });
                                    row.col(|ui| {
                                        if let Some(event) =
                                            super::field_addition::design::data_type(
                                                ui,
                                                presentation,
                                                addition_kind,
                                            )
                                        {
                                            addition_event = Some(event);
                                        }
                                    });
                                    row.col(|ui| {
                                        grid::cell(ui, "", grid::CellStyle::Value).on_hover_text(
                                            crate::localization::tr(ui, "ribbon-unavailable"),
                                        );
                                    });
                                } else {
                                    for _ in 0..3 {
                                        row.col(grid::blank);
                                    }
                                }
                                return;
                            };
                            row.col(|ui| {
                                grid::selector(
                                    ui,
                                    !addition_pending && position == self.selected_field,
                                )
                            });
                            row.col(|ui| {
                                let response = if let Some(editor) = &rename_editor
                                    && editor.target.column == field.name
                                    && editor.target.native_number == field.number
                                {
                                    let response = ui
                                        .scope(|ui| super::field_rename::widget::show(ui, editor));
                                    saved_field_event = response
                                        .inner
                                        .map(super::saved_field_header::Event::Rename);
                                    response.response
                                } else {
                                    let pending = pending_drop.as_ref().is_some_and(|target| {
                                        target.column == field.name
                                            && target.native_number == field.number
                                    });
                                    let pending_label = pending.then(|| {
                                        crate::localization::message(
                                            ui,
                                            "field-drop-pending",
                                            &[("column", field.name.as_str())],
                                        )
                                    });
                                    let response = grid::cell(
                                        ui,
                                        pending_label.as_deref().unwrap_or(&field.name),
                                        grid::CellStyle::FieldName {
                                            selected: !addition_pending
                                                && position == self.selected_field,
                                            id: field_focus_id(
                                                focus_scope,
                                                document_revision,
                                                &schema.table,
                                                position,
                                                field,
                                            ),
                                        },
                                    );
                                    if let Some(tab) = &rename_tab {
                                        super::saved_field_header::entry_menu(
                                            ui,
                                            &response,
                                            tab,
                                            &field.name,
                                            Some(field.number),
                                            rename_enabled,
                                            drop_enabled,
                                            &mut saved_field_event,
                                        );
                                    }
                                    if drop_enabled
                                        && response.has_focus()
                                        && ui.input_mut(|input| {
                                            input.consume_key(
                                                egui::Modifiers::NONE,
                                                egui::Key::Delete,
                                            )
                                        })
                                        && let Some(tab) = &rename_tab
                                    {
                                        saved_field_event = Some(
                                            match super::FieldDropTarget::from_parts(
                                                tab,
                                                &field.name,
                                                field.number,
                                            ) {
                                                Some(target) => {
                                                    super::saved_field_header::Event::Drop(target)
                                                }
                                                None => {
                                                    super::saved_field_header::Event::MemoryLimit
                                                }
                                            },
                                        );
                                    }
                                    response
                                };
                                if position == self.selected_field {
                                    if enter_grid {
                                        response.request_focus();
                                        response.scroll_to_me(None);
                                        entered_grid = true;
                                    }
                                    focus::register(ui, Pane::Document, &response, true);
                                }
                                let activated = ui.is_enabled()
                                    && (response.clicked()
                                        || response.has_focus() && response.gained_focus());
                                if activated && self.selected_field != position {
                                    self.selected_field = position;
                                    ui.ctx().request_repaint();
                                } else if response.has_focus() && self.selected_field != position {
                                    // A newly published schema resets its selection. The old
                                    // focused field must not silently reclaim that selection.
                                    response.surrender_focus();
                                }
                            });
                            row.col(|ui| {
                                let label = type_labels::display(&localizer, field);
                                let response = grid::cell(ui, &label.text, grid::CellStyle::Value);
                                if label.reader_fallback {
                                    response.on_hover_text(localizer.text("design-carrier-help"));
                                }
                            });
                            row.col(|ui| {
                                let value = field
                                    .properties
                                    .iter()
                                    .find(|property| property.name == "Description")
                                    .map(|property| &property.value);
                                let (description, style) = match value {
                                    Some(FieldPropertyValue::Text(value)) => {
                                        (Cow::Borrowed(value.as_str()), grid::CellStyle::Value)
                                    }
                                    Some(FieldPropertyValue::Absent) => {
                                        (Cow::Borrowed(""), grid::CellStyle::Value)
                                    }
                                    Some(FieldPropertyValue::Uninspected) | None => (
                                        Cow::Owned(localizer.text("design-not-inspected")),
                                        grid::CellStyle::Unknown,
                                    ),
                                    Some(
                                        FieldPropertyValue::Unsupported
                                        | FieldPropertyValue::Boolean(_),
                                    ) => (
                                        Cow::Owned(localizer.text("design-unsupported-value")),
                                        grid::CellStyle::Unknown,
                                    ),
                                };
                                grid::cell(ui, &description, style);
                            });
                        });
                    });
            },
        );
        pane_splitter(ui, grid_height);
        if let Some(mut entry) = entry
            && !entered_grid
        {
            // TableBuilder applies an offscreen scroll target before the next draw.
            // Keep only this still-current logical request for that adjacent frame.
            entry.last_frame = ui.ctx().cumulative_frame_nr();
            self.design_focus_pending = Some(entry);
            ui.ctx().request_repaint();
        }
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(crate::localization::tr(ui, "design-field-properties")).size(11.0),
            );
        });
        if addition
            .as_ref()
            .is_some_and(|(_, presentation)| presentation.name.is_some())
        {
            if let Some((_, presentation)) = &addition {
                self.show_field_addition_properties(
                    ui,
                    presentation.enabled,
                    &presentation.choices,
                );
            }
        } else if let Some(field) = schema.fields.get(self.selected_field) {
            ui.push_id(
                (
                    document_revision,
                    &schema.table,
                    self.selected_field,
                    field.number,
                    &field.name,
                    self.focus_publication,
                ),
                |ui| properties::show(ui, field),
            );
        }
        if let Some((tab, _)) = &addition {
            self.apply_design_field_addition(tab, addition_event);
        }
        self.apply_saved_field_event(
            saved_field_event,
            rename_editor.as_ref().map(|editor| &editor.target),
            rename_editor.as_ref().is_some_and(|editor| editor.enabled),
        );
    }
}

/// Keep a presentation-only split per object through its existing parent UI ID.
fn pane_height(ui: &Ui) -> f32 {
    let maximum =
        (ui.available_height() - MIN_PROPERTIES_HEIGHT - SPLITTER_HEIGHT).max(MIN_GRID_HEIGHT);
    let id = ui.make_persistent_id("schema_grid_height");
    ui.data(|data| data.get_temp::<f32>(id))
        .unwrap_or(ui.available_height() * 0.68)
        .clamp(MIN_GRID_HEIGHT, maximum)
}

fn pane_splitter(ui: &mut Ui, height: f32) {
    let id = ui.make_persistent_id("schema_grid_height");
    let maximum = (height + ui.available_height() - MIN_PROPERTIES_HEIGHT - SPLITTER_HEIGHT)
        .max(MIN_GRID_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SPLITTER_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let label = crate::localization::tr(ui, "design-resize-panes");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
    });
    let keyboard_delta = if response.has_focus() {
        ui.input_mut(|input| {
            let down = input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
            let up = input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
            (i32::from(down) - i32::from(up)) as f32 * FIELD_ROW_HEIGHT
        })
    } else {
        0.0
    };
    if response.clicked() {
        response.request_focus();
    }
    if response.dragged() || keyboard_delta != 0.0 {
        let next =
            (height + response.drag_delta().y + keyboard_delta).clamp(MIN_GRID_HEIGHT, maximum);
        ui.data_mut(|data| data.insert_temp(id, next));
        ui.ctx().request_repaint();
    }
    let palette = crate::theme::palette(ui.ctx());
    let color = if response.hovered() || response.has_focus() {
        palette.accent
    } else {
        palette.grid_line
    };
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, color),
    );
}

fn field_focus_id(
    scope: Id,
    document_revision: Option<u64>,
    table: &str,
    position: usize,
    field: &Field,
) -> Id {
    scope.with((
        "design_field",
        document_revision,
        table,
        position,
        field.number,
        field.name.as_str(),
    ))
}

/// Move only the currently focused field in the currently displayed declaration.
/// This produces no row handle, edit capability or host action.
fn navigate_fields(
    ui: &mut Ui,
    scope: Id,
    document_revision: Option<u64>,
    schema: &Schema,
    selected: &mut usize,
) -> Option<usize> {
    if !ui.is_enabled() {
        return None;
    }
    let field = schema.fields.get(*selected)?;
    let current_id = field_focus_id(scope, document_revision, &schema.table, *selected, field);
    let previous = ui.ctx().read_response(current_id)?;
    if !previous.has_focus() {
        return None;
    }
    let pressed_elsewhere = ui.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|position| !previous.interact_rect.contains(position))
    });
    if pressed_elsewhere || previous.clicked_elsewhere() {
        previous.surrender_focus();
        return None;
    }

    let before = *selected;
    let mut consumed = false;
    ui.input_mut(|input| {
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            if !modifiers.is_none() {
                return true;
            }
            match key {
                egui::Key::ArrowUp => *selected = selected.saturating_sub(1),
                egui::Key::ArrowDown => {
                    *selected = selected.saturating_add(1).min(schema.fields.len() - 1);
                }
                _ => return true,
            }
            consumed = true;
            false
        });
    });
    if consumed {
        // egui schedules cardinal focus movement before rendering widgets.
        // Cancel only the move corresponding to the keys owned above.
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
    }
    if before == *selected {
        return None;
    }

    let target = &schema.fields[*selected];
    let target_id = field_focus_id(scope, document_revision, &schema.table, *selected, target);
    ui.memory_mut(|memory| memory.request_focus(target_id));
    ui.ctx().request_repaint();
    Some(*selected)
}

fn property(ui: &mut Ui, label: &str, value: impl Into<egui::WidgetText>) {
    let name = ui.label(label);
    ui.add(egui::Label::new(value).selectable(true))
        .labelled_by(name.id);
    ui.end_row();
}

fn yes_no(ui: &Ui, value: bool) -> String {
    crate::localization::tr(ui, if value { "shell-yes" } else { "shell-no" })
}

#[cfg(test)]
mod tests;
