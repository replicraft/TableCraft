//! Read-only Table Design uses host-supplied facts without interpreting storage.
mod grid;
mod properties;
pub use properties::{FieldProperty, FieldPropertyValue};

use super::{DocumentView, State};
use crate::workspace::focus::{self, Pane};
use egui::{Id, RichText, ScrollArea, Ui};
use egui_extras::{Column, TableBuilder};

const MAX_PANE_ENTRY_FRAMES: u64 = 8;

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
    pub scalar_type: Option<String>,
    /// Optional host-supplied Access presentation name, never inferred from flags.
    pub display_type: Option<String>,
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
            table: schema.table.clone(),
            position: self.selected_field,
            field_number: field.number,
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
        let localizer = self.localizer.clone();
        let Some(schema) = &self.schema else {
            ui.weak(crate::localization::tr(ui, "design-choose"));
            return;
        };
        ui.small(crate::localization::tr(ui, "design-read-only"))
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

        egui::Resize::default()
            .id_salt("schema_fields")
            .default_width(ui.available_width())
            .min_width(ui.available_width())
            .max_width(ui.available_width())
            .default_height((ui.available_height() * 0.68).max(120.0))
            .min_height(90.0)
            .max_height((ui.available_height() - 140.0).max(90.0))
            .resizable([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let mut table = TableBuilder::new(ui).animate_scrolling(false);
                if let Some(position) = scroll_to {
                    table = table.scroll_to_row(position, None);
                }
                table
                    .striped(false)
                    .resizable(true)
                    .column(Column::initial(210.0).at_least(100.0))
                    .column(Column::initial(140.0).at_least(90.0))
                    .column(Column::remainder().at_least(70.0))
                    .header(22.0, |mut header| {
                        for (position, title) in [
                            localizer.text("design-field-name"),
                            localizer.text("design-data-type"),
                            localizer.text("design-description"),
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
                        body.rows(22.0, schema.fields.len(), |mut row| {
                            let position = row.index();
                            let field = &schema.fields[position];
                            row.col(|ui| {
                                let response = grid::cell(
                                    ui,
                                    &field.name,
                                    grid::CellStyle::FieldName {
                                        selected: position == self.selected_field,
                                        id: field_focus_id(
                                            focus_scope,
                                            document_revision,
                                            &schema.table,
                                            position,
                                            field,
                                        ),
                                    },
                                );
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
                                let fallback = crate::localization::tr(ui, "design-unsupported");
                                grid::cell(
                                    ui,
                                    field
                                        .display_type
                                        .as_deref()
                                        .or(field.scalar_type.as_deref())
                                        .unwrap_or(&fallback),
                                    grid::CellStyle::Value,
                                );
                            });
                            row.col(|ui| {
                                let description = field
                                    .properties
                                    .iter()
                                    .find_map(|property| match (&property.name, &property.value) {
                                        (&"Description", FieldPropertyValue::Text(value)) => {
                                            Some(value.as_str())
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or("");
                                grid::cell(ui, description, grid::CellStyle::Value);
                            });
                        });
                    });
            });
        if let Some(mut entry) = entry
            && !entered_grid
        {
            // TableBuilder applies an offscreen scroll target before the next draw.
            // Keep only this still-current logical request for that adjacent frame.
            entry.last_frame = ui.ctx().cumulative_frame_nr();
            self.design_focus_pending = Some(entry);
            ui.ctx().request_repaint();
        }
        ui.separator();
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(crate::localization::tr(ui, "design-field-properties")).size(11.0),
            );
        });
        let Some(field) = schema.fields.get(self.selected_field) else {
            return;
        };
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            ScrollArea::vertical()
                .id_salt("field_properties")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    ui.push_id(
                        (
                            document_revision,
                            &schema.table,
                            self.selected_field,
                            field.number,
                        ),
                        |ui| {
                            properties::show(ui, field);
                        },
                    );
                });
        });
    }
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

fn property(ui: &mut Ui, label: &str, value: impl Into<String>) {
    let name = ui.label(label);
    ui.add(egui::Label::new(value.into()).selectable(true))
        .labelled_by(name.id);
    ui.end_row();
}

fn yes_no(ui: &Ui, value: bool) -> String {
    crate::localization::tr(ui, if value { "shell-yes" } else { "shell-no" })
}

#[cfg(test)]
mod tests;
