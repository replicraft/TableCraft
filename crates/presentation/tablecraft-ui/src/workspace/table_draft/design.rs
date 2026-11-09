//! Editable initial declarations share the same unsaved model as Datasheet view.
mod activation;
pub(in crate::workspace::table_draft) mod description;
pub(super) mod new_field;
pub(in crate::workspace::table_draft) mod properties;
#[cfg(test)]
mod tests;
mod types;

use super::{Editor, grid::Event, rename};
use crate::localization::{message, tr};
use crate::workspace::{DocumentView, ObjectKind, State, ribbon::Icon};
use egui::{Ui, WidgetInfo, WidgetType};
use egui_extras::{Column, TableBuilder};
use tablecraft_builders::table::{
    DraftField, DraftFieldId, DraftFieldType, MAX_DRAFT_FIELDS, TableDraft,
};

const ROW_HEIGHT: f32 = 22.0;
const SELECTOR_WIDTH: f32 = 22.0;
const NAME_WIDTH: f32 = 192.0;
const TYPE_WIDTH: f32 = 160.0;
const MIN_NAME_WIDTH: f32 = 100.0;
const MIN_TYPE_WIDTH: f32 = 100.0;
const MIN_DESCRIPTION_WIDTH: f32 = 100.0;
const PROPERTIES_HEIGHT: f32 = 180.0;
const MIN_PROPERTIES_HEIGHT: f32 = 100.0;
// Header, generated ID, one ordinary field and the inline blank row remain visible.
const MIN_GRID_ROWS: f32 = 4.0;
const FOOTER_PADDING: f32 = 12.0;
const KEY_SIZE: f32 = 12.0;
const SELECTOR_HALF_WIDTH: f32 = 3.0;
const SELECTOR_HALF_HEIGHT: f32 = 4.0;

#[derive(Default)]
pub(in crate::workspace) struct Design {
    pub(in crate::workspace) active: bool,
    pub(super) requested: Option<bool>,
    pub(super) delete_requested: Option<DraftFieldId>,
    selected: Option<DraftFieldId>,
    lookup: bool,
    pending_focus: Option<(DraftFieldId, activation::Target)>,
}

impl Design {
    pub(super) fn new(active: bool) -> Self {
        Self {
            active,
            ..Self::default()
        }
    }

    pub(super) fn select(&mut self, field: DraftFieldId) {
        self.selected = Some(field);
    }

    pub(super) fn selected_field<'a>(&self, draft: &'a TableDraft) -> Option<&'a DraftField> {
        draft
            .fields()
            .iter()
            .find(|field| Some(field.id()) == self.selected)
            .or_else(|| draft.fields().first())
    }
}

impl State {
    pub fn table_draft_design_visible(&self) -> bool {
        self.table_draft
            .as_ref()
            .is_some_and(|editor| editor.design.active)
            && self
                .active_object()
                .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
    }

    pub fn show_table_draft_design(&mut self) -> bool {
        self.select_table_draft_view(true)
    }

    pub fn show_table_draft_datasheet(&mut self) -> bool {
        self.select_table_draft_view(false)
    }

    pub(in crate::workspace) fn queue_table_draft_view(&mut self, design: bool) -> bool {
        if !self.object_switch_ready()
            || self.creation_is_pending()
            || !self
                .active_object()
                .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
        {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        if editor.save_blocked {
            return false;
        }
        editor.design.requested = Some(design);
        true
    }

    fn select_table_draft_view(&mut self, design: bool) -> bool {
        if !self.object_switch_ready()
            || self.creation_is_pending()
            || !self
                .active_object()
                .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
        {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        if editor.save_blocked || !editor.commit_pending() {
            return false;
        }
        editor.design.active = design;
        editor.design.requested = None;
        self.view = if design {
            DocumentView::Design
        } else {
            DocumentView::Table
        };
        self.finish_object_activation();
        true
    }

    /// Change an unsaved declaration without converting, clearing or replaying input.
    pub fn set_table_draft_field_type(
        &mut self,
        field: DraftFieldId,
        kind: DraftFieldType,
    ) -> bool {
        if !self.object_switch_ready()
            || self.creation_is_pending()
            || !self
                .active_object()
                .is_some_and(|tab| matches!(tab.target.kind, ObjectKind::DraftTable { .. }))
        {
            return false;
        }
        let Some(editor) = &mut self.table_draft else {
            return false;
        };
        if editor.save_blocked
            || editor.rename.is_some()
            || editor.new_field.pending()
            || editor.properties.field_rejected(field)
        {
            return false;
        }
        let Some(current) = editor
            .draft
            .fields()
            .iter()
            .find(|current| current.id() == field)
        else {
            return false;
        };
        if current.is_protected() || kind == DraftFieldType::AutoNumberIncrement {
            return false;
        }
        if current.kind() == kind {
            return true;
        }
        let Some((position, revision)) = editor.record.type_change(field) else {
            return false;
        };
        if editor.draft.set_field_type(field, kind).is_err() {
            return false;
        }
        editor.record.change_type_prepared(position, revision, kind);
        true
    }
}

pub(super) fn type_key(kind: DraftFieldType) -> &'static str {
    match kind {
        DraftFieldType::AutoNumberIncrement => "design-type-autonumber",
        DraftFieldType::ShortText => "design-type-short-text",
        DraftFieldType::Number => "design-type-number",
        DraftFieldType::LargeInteger => "design-type-large-number",
        DraftFieldType::Currency => "design-type-currency",
        DraftFieldType::DateTime => "design-type-date-time",
        DraftFieldType::YesNo => "design-type-yes-no",
    }
}

pub(super) fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    can_save: bool,
    can_delete: bool,
) -> Option<Event> {
    if !editor.record.synchronize(&editor.draft) || !editor.descriptions.synchronize(&editor.draft)
    {
        ui.colored_label(ui.visuals().error_fg_color, tr(ui, "insertion-input-limit"));
        return None;
    }
    let mut event = None;
    let mut save = false;
    egui::Panel::bottom(ui.make_persistent_id("draft_design_footer"))
        .exact_size(footer_height(ui, editor))
        .show(ui, |ui| {
            ui.weak(tr(ui, "table-draft-design-input-help"));
            if editor.error || editor.new_field.rejected {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    tr(ui, "table-draft-invalid-name"),
                );
            }
            if editor.descriptions.rejected() {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    tr(ui, "table-draft-description-invalid"),
                );
            }
            if editor.properties.rejected() {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    tr(ui, "table-draft-property-invalid"),
                );
            }
            if editor.record.has_rejected_input() {
                ui.colored_label(ui.visuals().error_fg_color, tr(ui, "insertion-input-limit"));
            }
            if editor.save_blocked {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    tr(ui, "table-draft-save-blocked"),
                );
            }
            save = ui
                .add_enabled(can_save, egui::Button::new(tr(ui, "table-draft-save")))
                .clicked();
        });
    let properties_limit =
        (ui.available_height() - MIN_GRID_ROWS * ROW_HEIGHT).max(MIN_PROPERTIES_HEIGHT);
    let properties_minimum = properties::minimum_height(ui, editor)
        .max(MIN_PROPERTIES_HEIGHT)
        .min(properties_limit);
    egui::Panel::bottom(ui.make_persistent_id("draft_design_properties"))
        .resizable(true)
        .default_size(PROPERTIES_HEIGHT.max(properties_minimum))
        .min_size(properties_minimum)
        .max_size(properties_limit)
        .show(ui, |ui| properties::show(ui, editor));

    let defer_existing = editor.new_field.editing() || editor.rename.is_some();
    let mut activation_request = None;
    let mut new_field_commit = false;
    let mut rename_outcome = None;
    let grid_rect = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("initial_design_grid_viewport")
            .max_rect(grid_rect),
        |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(grid_rect));
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            let table_height = (ui.available_height() - ROW_HEIGHT).max(0.0);
            let mut columns = Vec::new();
            let mut content_bottom = grid_rect.top();
            let mut table = TableBuilder::new(ui)
                .id_salt("initial_design_fields")
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .striped(false)
                .resizable(true)
                .column(Column::exact(SELECTOR_WIDTH).resizable(false))
                .column(
                    Column::initial(NAME_WIDTH)
                        .at_least(MIN_NAME_WIDTH)
                        .clip(true),
                )
                .column(
                    Column::initial(TYPE_WIDTH)
                        .at_least(MIN_TYPE_WIDTH)
                        .clip(true),
                )
                .column(
                    Column::remainder()
                        .at_least(MIN_DESCRIPTION_WIDTH)
                        .clip(true),
                );
            table = table.max_scroll_height(table_height);
            table
                .header(ROW_HEIGHT, |mut row| {
                    row.col(|ui| {
                        columns.push(ui.max_rect());
                        crate::datasheet::grid_cell(ui, true);
                    });
                    for key in [
                        "design-field-name",
                        "design-data-type",
                        "design-description-optional",
                    ] {
                        row.col(|ui| {
                            columns.push(ui.max_rect());
                            crate::datasheet::grid_cell(ui, true);
                            ui.centered_and_justified(|ui| ui.label(tr(ui, key)));
                        });
                    }
                })
                .body(|mut body| {
                    for field in editor.draft.fields() {
                        body.row(ROW_HEIGHT, |mut row| {
                            row.col(|ui| {
                                crate::datasheet::grid_cell(ui, true);
                                let selected = editor.design.selected.or_else(|| {
                                    editor.draft.fields().first().map(|field| field.id())
                                }) == Some(field.id());
                                let (rect, response) = ui
                                    .allocate_exact_size(ui.available_size(), egui::Sense::click());
                                response.widget_info(|| {
                                    WidgetInfo::selected(
                                        WidgetType::Button,
                                        ui.is_enabled(),
                                        selected,
                                        field.name(),
                                    )
                                });
                                if field.is_primary() {
                                    Icon::Key.image(ui, KEY_SIZE).paint_at(
                                        ui,
                                        egui::Rect::from_center_size(
                                            rect.center(),
                                            egui::Vec2::splat(KEY_SIZE),
                                        ),
                                    );
                                } else if selected {
                                    let center = rect.center();
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![
                                            center
                                                + egui::vec2(
                                                    -SELECTOR_HALF_WIDTH,
                                                    -SELECTOR_HALF_HEIGHT,
                                                ),
                                            center + egui::vec2(SELECTOR_HALF_WIDTH, 0.0),
                                            center
                                                + egui::vec2(
                                                    -SELECTOR_HALF_WIDTH,
                                                    SELECTOR_HALF_HEIGHT,
                                                ),
                                        ],
                                        ui.visuals().text_color(),
                                        egui::Stroke::NONE,
                                    ));
                                }
                                if can_delete
                                    && editor.design.selected == Some(field.id())
                                    && response.has_focus()
                                    && response.enabled()
                                    && !egui::Popup::is_any_open(ui.ctx())
                                    && consume_unmodified_delete(ui)
                                {
                                    event = Some(Event::DeleteField(field.id()));
                                }
                                if response.clicked() {
                                    activation_request = Some(activation::Request {
                                        field: field.id(),
                                        target: activation::Target::Selector,
                                        response,
                                    });
                                }
                            });
                            row.col(|ui| {
                                crate::datasheet::grid_cell(ui, false);
                                if let Some(rename) = &mut editor.rename
                                    && rename.field == field.id()
                                {
                                    rename_outcome = rename.show(ui);
                                    editor.error |= rename.rejected;
                                } else {
                                    let response = ui.add_enabled(
                                        !field.is_protected() && editor.rename.is_none(),
                                        egui::Button::new(field.name()).frame(false).truncate(),
                                    );
                                    let label = message(
                                        ui,
                                        "table-draft-design-name",
                                        &[("field", field.name())],
                                    );
                                    response.widget_info(|| {
                                        WidgetInfo::labeled(
                                            WidgetType::Button,
                                            response.enabled(),
                                            &label,
                                        )
                                    });
                                    if response.clicked() {
                                        activation_request = Some(activation::Request {
                                            field: field.id(),
                                            target: activation::Target::Name,
                                            // Keyboard/focus/activation registries retain their
                                            // own response while the original response remains
                                            // needed for hover, focus, click state or returned
                                            // geometry.
                                            response: response.clone(),
                                        });
                                    }
                                    if field.is_protected() {
                                        response.on_disabled_hover_text(tr(
                                            ui,
                                            "table-draft-design-protected",
                                        ));
                                    }
                                }
                            });
                            row.col(|ui| {
                                crate::datasheet::grid_cell(ui, false);
                                ui.add_enabled_ui(!field.is_protected(), |ui| {
                                    let label = message(
                                        ui,
                                        "table-draft-design-type",
                                        &[("field", field.name())],
                                    );
                                    if defer_existing {
                                        let response = activation::type_proxy(
                                            ui,
                                            &tr(ui, type_key(field.kind())),
                                            &label,
                                        );
                                        if response.clicked() {
                                            activation_request = Some(activation::Request {
                                                field: field.id(),
                                                target: activation::Target::Type,
                                                response,
                                            });
                                        }
                                        return;
                                    }
                                    let response =
                                        egui::ComboBox::from_id_salt(("draft_type", field.id()))
                                            .selected_text(tr(ui, type_key(field.kind())))
                                            .width(ui.available_width())
                                            .height(types::menu_height(ui))
                                            .show_ui(ui, |ui| {
                                                if let Some(kind) =
                                                    types::show(ui, field.kind(), |kind| {
                                                        !field.is_primary()
                                                            || field.supports_primary_type(kind)
                                                    })
                                                {
                                                    event =
                                                        Some(Event::ChangeType(field.id(), kind));
                                                }
                                            })
                                            .response;
                                    response.widget_info(|| {
                                        WidgetInfo::labeled(
                                            WidgetType::ComboBox,
                                            response.enabled(),
                                            &label,
                                        )
                                    });
                                    if response.enabled()
                                        && editor.design.pending_focus
                                            == Some((field.id(), activation::Target::Type))
                                    {
                                        egui::Popup::open_id(
                                            ui.ctx(),
                                            // egui 0.36.2 derives its popup from the current button
                                            // response with this salt. No prior-frame ID is replayed.
                                            response.id.with("popup"),
                                        );
                                        response.request_focus();
                                        editor.design.pending_focus = None;
                                        ui.ctx().request_repaint();
                                    }
                                    if response.clicked() {
                                        activation_request = Some(activation::Request {
                                            field: field.id(),
                                            target: activation::Target::Selector,
                                            // Keyboard/focus/activation registries retain their
                                            // own response while the original response remains
                                            // needed for hover, focus, click state or returned
                                            // geometry.
                                            response: response.clone(),
                                        });
                                    }
                                    response.on_disabled_hover_text(tr(
                                        ui,
                                        "table-draft-design-protected",
                                    ));
                                });
                            });
                            row.col(|ui| {
                                crate::datasheet::grid_cell(ui, false);
                                if defer_existing {
                                    let response = editor.descriptions.show_inert(ui, field);
                                    if response.clicked() {
                                        activation_request = Some(activation::Request {
                                            field: field.id(),
                                            target: activation::Target::Description,
                                            response,
                                        });
                                    }
                                } else {
                                    let available = editor.properties.raw_edit_available();
                                    let focus = ui.is_enabled()
                                        && available
                                        && editor.design.pending_focus
                                            == Some((field.id(), activation::Target::Description));
                                    let output = ui
                                        .add_enabled_ui(available, |ui| {
                                            editor.descriptions.show(ui, field, focus)
                                        })
                                        .inner;
                                    if focus {
                                        editor.design.pending_focus = None;
                                    }
                                    if let Some(output) = output {
                                        if output.response.clicked()
                                            || (!focus && output.response.gained_focus())
                                        {
                                            activation_request = Some(activation::Request {
                                                field: field.id(),
                                                target: activation::Target::Selector,
                                                response: output.response,
                                            });
                                        }
                                        if output.changed && !editor.properties.record_raw_edit() {
                                            editor.descriptions.reject(field.id());
                                        }
                                    }
                                }
                            });
                        });
                    }
                    body.row(ROW_HEIGHT, |mut row| {
                        row.col(|ui| {
                            content_bottom = ui.max_rect().bottom();
                            crate::datasheet::grid_cell(ui, true);
                        });
                        row.col(|ui| {
                            crate::datasheet::grid_cell(ui, false);
                            let available = editor.draft.fields().len() < MAX_DRAFT_FIELDS
                                && editor.rename.is_none()
                                && editor.properties.raw_edit_available();
                            ui.add_enabled_ui(available, |ui| {
                                new_field_commit |=
                                    editor.new_field.show_name(ui, &mut editor.properties);
                            });
                        });
                        row.col(|ui| {
                            crate::datasheet::grid_cell(ui, false);
                            let available = editor.draft.fields().len() < MAX_DRAFT_FIELDS
                                && editor.rename.is_none()
                                && editor.properties.raw_edit_available();
                            ui.add_enabled_ui(available, |ui| {
                                new_field_commit |=
                                    editor.new_field.show_type(ui, &mut editor.properties);
                            });
                        });
                        row.col(|ui| crate::datasheet::grid_cell(ui, false));
                    });
                });
            if let Some(selector) = columns.first() {
                let unoccupied = egui::Rect::from_min_max(
                    egui::pos2(selector.left(), content_bottom),
                    egui::pos2(selector.right(), grid_rect.bottom()),
                )
                .intersect(grid_rect);
                if unoccupied.is_positive() {
                    ui.painter().rect_filled(
                        unoccupied,
                        0.0,
                        crate::theme::palette(ui.ctx()).grid_header,
                    );
                }
            }
            ui.painter().add(crate::datasheet::empty_grid::shape(
                &columns,
                grid_rect,
                content_bottom,
                ROW_HEIGHT,
                crate::theme::palette(ui.ctx()).grid_line,
            ));
        },
    );
    match rename_outcome {
        Some(rename::Outcome::Commit) if activation_request.is_none() => {
            editor.commit_rename();
        }
        Some(rename::Outcome::Commit) => {}
        Some(rename::Outcome::Cancel) => {
            editor.rename = None;
            editor.error = false;
        }
        None => {}
    }
    if let Some(request) = activation_request {
        if !activation::apply(editor, request) {
            event = None;
        }
    } else if new_field_commit {
        editor.commit_new_field();
    }
    editor.descriptions.commit(&mut editor.draft);
    let requested = std::mem::take(&mut editor.save_requested);
    if event.is_none() && (save || requested && can_save) {
        event = Some(Event::Save);
    }
    event
}

fn footer_height(ui: &Ui, editor: &Editor) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut keys = vec!["table-draft-design-input-help"];
    if editor.error || editor.new_field.rejected {
        keys.push("table-draft-invalid-name");
    }
    if editor.descriptions.rejected() {
        keys.push("table-draft-description-invalid");
    }
    if editor.properties.rejected() {
        keys.push("table-draft-property-invalid");
    }
    if editor.record.has_rejected_input() {
        keys.push("insertion-input-limit");
    }
    if editor.save_blocked {
        keys.push("table-draft-save-blocked");
    }
    FOOTER_PADDING
        + ui.spacing().interact_size.y
        + keys
            .iter()
            .map(|key| {
                ui.painter()
                    .layout(
                        tr(ui, key),
                        // The layout API consumes this FontId; keep the original style or
                        // loop-local descriptor available for the next measurement.
                        font.clone(),
                        ui.visuals().text_color(),
                        ui.available_width(),
                    )
                    .size()
                    .y
                    + ui.spacing().item_spacing.y
            })
            .sum::<f32>()
}

// consume_key intentionally allows extra Shift/Alt modifiers. Row deletion
// requires the exact event chord, independent of the frame's final key state.
fn consume_unmodified_delete(ui: &Ui) -> bool {
    ui.input_mut(|input| {
        let mut consumed = false;
        input.events.retain(|event| {
            if matches!(event, egui::Event::Key {
                key: egui::Key::Delete,
                pressed: true,
                modifiers,
                ..
            } if *modifiers == egui::Modifiers::NONE)
            {
                consumed = true;
                false
            } else {
                true
            }
        });
        consumed
    })
}
