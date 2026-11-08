use super::Editor;
use crate::theme::palette;
use crate::workspace::ribbon::Icon;
use crate::{datasheet::grid_cell, localization::tr};
use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};
use tablecraft_builders::table::{DraftFieldId, DraftFieldType, MAX_DRAFT_FIELDS};

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;
const FIELD_WIDTH: f32 = 150.0;
const SELECTOR_WIDTH: f32 = 22.0;
const PRIMARY_ICON_SIZE: f32 = 12.0;
const PRIMARY_ICON_GAP: f32 = 4.0;
const FOOTER_PADDING: f32 = 12.0;
const ADD_CHEVRON_INSET: f32 = 8.0;
const ADD_LABEL_PADDING: f32 = 4.0;
const ADD_CHEVRON_GUTTER: f32 = 20.0;
const ADD_CHEVRON_HALF_WIDTH: f32 = 3.0;
const ADD_CHEVRON_HALF_HEIGHT: f32 = 1.5;

pub(super) enum Event {
    Save,
    DeleteField(DraftFieldId),
    AddField(DraftFieldType),
    ChangeType(DraftFieldId, DraftFieldType),
}

pub(super) fn show(ui: &mut Ui, editor: &mut Editor, can_save: bool) -> Option<Event> {
    if !editor.record.synchronize(&editor.draft) {
        ui.colored_label(ui.visuals().error_fg_color, tr(ui, "insertion-input-limit"));
        return None;
    }
    let colors = palette(ui.ctx());
    let footer_height = footer_height(ui, editor);
    let grid_height = (ui.available_height() - footer_height).max(0.0);
    let scrollbar_height = egui::style::ScrollStyle::solid().bar_width;
    let body_height = (grid_height - HEADER_HEIGHT - scrollbar_height).max(0.0);
    let mut addition = None;
    let mut rename = None;
    let mut delete_field = None;
    let mut rename_outcome = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        egui::ScrollArea::horizontal()
            .id_salt("table_draft_columns")
            .max_height(grid_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let viewport = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), grid_height),
                )
                .intersect(ui.clip_rect());
                let mut columns = Vec::with_capacity(editor.draft.fields().len() + 2);
                let mut content_bottom = viewport.top();
                let mut table = TableBuilder::new(ui)
                    .id_salt("table_draft_grid")
                    .max_scroll_height(body_height)
                    .min_scrolled_height(0.0)
                    .auto_shrink([false, false])
                    .striped(false)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(SELECTOR_WIDTH));
                for _ in editor.draft.fields() {
                    table = table.column(Column::initial(FIELD_WIDTH).resizable(true));
                }
                table = table.column(Column::exact(FIELD_WIDTH));
                table
                    .header(HEADER_HEIGHT, |mut header| {
                        header.col(|ui| {
                            columns.push(ui.max_rect());
                            grid_cell(ui, true);
                        });
                        for field in editor.draft.fields() {
                            header.col(|ui| {
                                columns.push(ui.max_rect());
                                grid_cell(ui, true);
                                if let Some(active) = &mut editor.rename
                                    && active.field == field.id()
                                {
                                    rename_outcome = active.show(ui);
                                    editor.error |= active.rejected;
                                    return;
                                }
                                if !field.is_primary() {
                                    match super::header::show(ui, field, editor.rename.is_none()) {
                                        Some(super::header::Command::Rename) => {
                                            rename = Some((field.id(), field.name().into()));
                                        }
                                        Some(super::header::Command::Delete) => {
                                            delete_field = Some(field.id());
                                        }
                                        None => {}
                                    }
                                    return;
                                }
                                let rect = ui.max_rect();
                                let inset = if field.is_primary() {
                                    let icon_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            rect.left() + PRIMARY_ICON_GAP,
                                            rect.center().y - PRIMARY_ICON_SIZE / 2.0,
                                        ),
                                        egui::Vec2::splat(PRIMARY_ICON_SIZE),
                                    );
                                    ui.put(icon_rect, Icon::Key.image(ui, PRIMARY_ICON_SIZE));
                                    PRIMARY_ICON_SIZE + PRIMARY_ICON_GAP
                                } else {
                                    0.0
                                };
                                let response = ui
                                    .put(
                                        rect.shrink2(egui::vec2(inset, 0.0)),
                                        egui::Label::new(RichText::new(field.name()).strong())
                                            .halign(egui::Align::Center)
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                    .on_hover_text(field.name());
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_role(egui::accesskit::Role::ColumnHeader);
                                    node.set_label(field.name());
                                });
                            });
                        }
                        header.col(|ui| {
                            columns.push(ui.max_rect());
                            grid_cell(ui, true);
                            let enabled = editor.draft.fields().len() < MAX_DRAFT_FIELDS
                                && editor.rename.is_none();
                            ui.add_enabled_ui(enabled, |ui| {
                                let label = tr(ui, "datasheet-click-to-add");
                                let button = egui::Button::new("")
                                    .min_size(ui.available_size())
                                    .frame(false);
                                let (response, _) =
                                    egui::containers::menu::MenuButton::from_button(button).ui(
                                        ui,
                                        |ui| {
                                            addition = super::gallery::show(ui);
                                        },
                                    );
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        &label,
                                    )
                                });
                                let text_rect = egui::Rect::from_min_max(
                                    response.rect.min + egui::vec2(ADD_LABEL_PADDING, 0.0),
                                    response.rect.max - egui::vec2(ADD_CHEVRON_GUTTER, 0.0),
                                );
                                let mut layout = egui::text::LayoutJob::simple(
                                    label,
                                    egui::TextStyle::Button.resolve(ui.style()),
                                    ui.visuals().widgets.inactive.fg_stroke.color,
                                    text_rect.width().max(0.0),
                                );
                                layout.wrap.max_rows = 1;
                                layout.wrap.break_anywhere = true;
                                for section in &mut layout.sections {
                                    section.format.italics = true;
                                }
                                let text = ui.painter().layout_job(layout);
                                let origin = egui::pos2(
                                    text_rect.center().x - text.size().x / 2.0,
                                    text_rect.center().y - text.size().y / 2.0,
                                );
                                ui.painter().with_clip_rect(text_rect).galley(
                                    origin,
                                    text,
                                    ui.visuals().widgets.inactive.fg_stroke.color,
                                );
                                let center = egui::pos2(
                                    response.rect.right() - ADD_CHEVRON_INSET,
                                    response.rect.center().y,
                                );
                                ui.painter().add(egui::Shape::line(
                                    vec![
                                        center
                                            + egui::vec2(
                                                -ADD_CHEVRON_HALF_WIDTH,
                                                -ADD_CHEVRON_HALF_HEIGHT,
                                            ),
                                        center + egui::vec2(0.0, ADD_CHEVRON_HALF_HEIGHT),
                                        center
                                            + egui::vec2(
                                                ADD_CHEVRON_HALF_WIDTH,
                                                -ADD_CHEVRON_HALF_HEIGHT,
                                            ),
                                    ],
                                    ui.visuals().widgets.inactive.fg_stroke,
                                ));
                                if editor.focus_add {
                                    response.request_focus();
                                    response.scroll_to_me(None);
                                    editor.focus_add = false;
                                } else if response.has_focus() {
                                    response.scroll_to_me(None);
                                }
                            });
                        });
                    })
                    .body(|mut body| {
                        body.row(ROW_HEIGHT, |mut row| {
                            row.col(|ui| {
                                content_bottom = ui.max_rect().bottom();
                                grid_cell(ui, true);
                                ui.label("*");
                            });
                            for field in editor.draft.fields() {
                                row.col(|ui| {
                                    grid_cell(ui, false);
                                    editor.record.show_cell(ui, field);
                                });
                            }
                            row.col(|ui| {
                                grid_cell(ui, false);
                            });
                        });
                    });
                // Use the same background rules as stored datasheets. There are
                // no additional table rows, widgets or native identities here.
                ui.painter().add(crate::datasheet::empty_grid::shape(
                    &columns,
                    viewport,
                    content_bottom,
                    ROW_HEIGHT,
                    colors.grid_line,
                ));
            });
    });
    if let Some(kind) = addition {
        match editor.draft.add_field(kind) {
            Ok(id) => {
                let name = editor
                    .draft
                    .fields()
                    .iter()
                    .find(|field| field.id() == id)
                    .unwrap()
                    .name();
                if !editor.record.synchronize(&editor.draft) {
                    editor.error = true;
                }
                editor.rename = Some(super::rename::Rename::new(id, name.into()));
                editor.focus_add = false;
                editor.error = false;
            }
            Err(_) => editor.error = true,
        }
    }
    if let Some(rename) = rename {
        editor.rename = Some(super::rename::Rename::new(rename.0, rename.1));
        editor.focus_add = false;
    }
    match rename_outcome {
        Some(super::rename::Outcome::Commit) => {
            editor.commit_rename();
        }
        Some(super::rename::Outcome::Cancel) => {
            editor.rename = None;
            editor.error = false;
            editor.focus_add = true;
        }
        None => {}
    }
    ui.add_space(8.0);
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
    if editor.error {
        ui.colored_label(
            ui.visuals().error_fg_color,
            tr(ui, "table-draft-invalid-name"),
        );
    }
    if editor.save_blocked {
        let response = ui.colored_label(
            ui.visuals().warn_fg_color,
            tr(ui, "table-draft-save-blocked"),
        );
        if let Some(message) = &editor.save_message {
            response.on_hover_text(message);
        }
    }
    ui.label(tr(ui, "table-draft-empty-note"));
    let clicked = ui
        .add_enabled(can_save, egui::Button::new(tr(ui, "table-draft-save")))
        .on_disabled_hover_text(tr(ui, "table-draft-save-unavailable"))
        .clicked();
    let requested = std::mem::take(&mut editor.save_requested);
    if let Some(field) = delete_field {
        Some(Event::DeleteField(field))
    } else if clicked || (requested && can_save) {
        Some(Event::Save)
    } else {
        None
    }
}

fn footer_height(ui: &Ui, editor: &Editor) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let line_height = ui.text_style_height(&egui::TextStyle::Body);
    let text_height = |key| {
        ui.painter()
            .layout(
                tr(ui, key),
                font.clone(),
                ui.visuals().text_color(),
                ui.available_width(),
            )
            .size()
            .y
    };
    let mut height = FOOTER_PADDING
        + text_height("table-draft-empty-note")
        + ui.spacing().interact_size.y.max(line_height)
        + ui.spacing().item_spacing.y * 2.0;
    if editor.descriptions.rejected() {
        height += text_height("table-draft-description-invalid") + ui.spacing().item_spacing.y;
    }
    if editor.properties.rejected() {
        height += text_height("table-draft-property-invalid") + ui.spacing().item_spacing.y;
    }
    if editor.record.has_rejected_input() {
        height += text_height("insertion-input-limit") + ui.spacing().item_spacing.y;
    }
    if editor.error {
        height += text_height("table-draft-invalid-name") + ui.spacing().item_spacing.y;
    }
    if editor.save_blocked {
        height += text_height("table-draft-save-blocked") + ui.spacing().item_spacing.y;
    }
    height
}
