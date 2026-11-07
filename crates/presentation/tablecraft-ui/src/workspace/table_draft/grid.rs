use super::Editor;
use crate::theme::palette;
use crate::workspace::ribbon::Icon;
use crate::{datasheet::grid_cell, localization::tr};
use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};
use tablecraft_builders::table::{DraftFieldType, MAX_DRAFT_FIELDS};

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;
const FIELD_WIDTH: f32 = 150.0;
const SELECTOR_WIDTH: f32 = 22.0;
const RENAME_WIDTH: f32 = 220.0;
const PRIMARY_ICON_SIZE: f32 = 12.0;
const PRIMARY_ICON_GAP: f32 = 4.0;
const NAME_INPUT_CHARACTERS: usize = 64;
const NAME_INPUT_BYTES: usize = 256;
const FOOTER_PADDING: f32 = 12.0;

pub(super) fn show(ui: &mut Ui, editor: &mut Editor, can_save: bool) -> bool {
    let colors = palette(ui.ctx());
    let footer_height = footer_height(ui, editor);
    let grid_height = (ui.available_height() - footer_height).max(0.0);
    let scrollbar_height = egui::style::ScrollStyle::solid().bar_width;
    let body_height = (grid_height - HEADER_HEIGHT - scrollbar_height).max(0.0);
    let mut addition = None;
    let mut rename = None;
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
                                if !field.is_primary() && response.double_clicked() {
                                    rename = Some((field.id(), field.name().into()));
                                }
                            });
                        }
                        header.col(|ui| {
                            columns.push(ui.max_rect());
                            grid_cell(ui, true);
                            ui.put(
                                ui.max_rect(),
                                egui::Label::new(
                                    RichText::new(tr(ui, "datasheet-click-to-add")).italics(),
                                )
                                .halign(egui::Align::Center)
                                .truncate(),
                            );
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
                                    if field.is_primary() {
                                        ui.label(
                                            RichText::new(tr(ui, "table-draft-new-record"))
                                                .color(colors.muted),
                                        );
                                    }
                                });
                            }
                            row.col(|ui| {
                                grid_cell(ui, false);
                                let enabled = editor.draft.fields().len() < MAX_DRAFT_FIELDS;
                                ui.add_enabled_ui(enabled, |ui| {
                                    let menu =
                                        ui.menu_button(tr(ui, "datasheet-click-to-add"), |ui| {
                                            if ui.button(tr(ui, "table-draft-short-text")).clicked()
                                            {
                                                addition = Some(DraftFieldType::ShortText);
                                                ui.close();
                                            }
                                            if ui
                                                .button(tr(ui, "table-draft-long-integer"))
                                                .clicked()
                                            {
                                                addition = Some(DraftFieldType::LongInteger);
                                                ui.close();
                                            }
                                        });
                                    if editor.focus_add {
                                        menu.response.request_focus();
                                        editor.focus_add = false;
                                    }
                                });
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
                editor.rename = Some((id, name.into()));
                editor.error = false;
            }
            Err(_) => editor.error = true,
        }
    }
    if let Some(rename) = rename {
        editor.rename = Some(rename);
    }
    ui.add_space(8.0);
    if let Some((id, name)) = &mut editor.rename {
        ui.horizontal(|ui| {
            let label = ui.label(tr(ui, "table-draft-field-name"));
            let input_id = ui.make_persistent_id("draft_field_name");
            let original = name.clone();
            let rejected = crate::text_input::reject_events_with_limits(
                ui,
                input_id,
                &original,
                NAME_INPUT_BYTES,
                NAME_INPUT_CHARACTERS,
            );
            ui.add(
                egui::TextEdit::singleline(name)
                    .id(input_id)
                    .desired_width(RENAME_WIDTH),
            )
            .labelled_by(label.id);
            if rejected
                || name.len() > NAME_INPUT_BYTES
                || name.chars().count() > NAME_INPUT_CHARACTERS
            {
                *name = original;
                editor.error = true;
            }
            if ui.button(tr(ui, "table-draft-apply-name")).clicked() {
                editor.error = editor.draft.rename_field(*id, name).is_err();
            }
        });
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
    ui.add_enabled(can_save, egui::Button::new(tr(ui, "table-draft-save")))
        .on_disabled_hover_text(tr(ui, "table-draft-save-unavailable"))
        .clicked()
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
    if editor.rename.is_some() {
        height += ui.spacing().interact_size.y.max(line_height) + ui.spacing().item_spacing.y;
    }
    if editor.error {
        height += text_height("table-draft-invalid-name") + ui.spacing().item_spacing.y;
    }
    if editor.save_blocked {
        height += text_height("table-draft-save-blocked") + ui.spacing().item_spacing.y;
    }
    height
}
