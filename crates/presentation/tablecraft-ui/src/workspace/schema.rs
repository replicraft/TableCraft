//! Read-only Table Design uses host-supplied facts without interpreting storage.
use super::State;
use egui::{RichText, ScrollArea, Ui};
use egui_extras::{Column, TableBuilder};

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
    pub reader_nullable: Option<bool>,
    pub decimal_parameters: Option<(u8, u8)>,
    pub complex_type_id: Option<u32>,
}

impl State {
    pub(super) fn design(&mut self, ui: &mut Ui) {
        let Some(schema) = &self.schema else {
            ui.weak("Choose View schema for a table to inspect its declaration.");
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(&schema.table);
            ui.label("Table Design · read only");
        });
        ui.small(format!(
            "Declared rows (unverified): {}",
            schema.declared_rows
        ));
        ui.small("Metadata only. Required, defaults, validation, descriptions and captions are not inspected.");
        ui.separator();
        if schema.fields.is_empty() {
            ui.weak("No fields in the captured declaration.");
            return;
        }

        egui::Resize::default()
            .id_salt("schema_fields")
            .default_height(250.0)
            .min_height(90.0)
            .max_height((ui.available_height() - 140.0).max(90.0))
            .resizable([false, true])
            .show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .column(Column::initial(210.0).at_least(100.0))
                    .column(Column::initial(140.0).at_least(90.0))
                    .column(Column::initial(160.0).at_least(100.0))
                    .column(Column::remainder().at_least(70.0))
                    .header(22.0, |mut header| {
                        for title in [
                            "Field Name",
                            "Declared Native Type",
                            "Readable Carrier",
                            "Storage Bytes",
                        ] {
                            header.col(|ui| {
                                ui.label(title);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(22.0, schema.fields.len(), |mut row| {
                            let position = row.index();
                            let field = &schema.fields[position];
                            row.set_selected(position == self.selected_field);
                            row.col(|ui| {
                                if ui
                                    .selectable_label(position == self.selected_field, &field.name)
                                    .clicked()
                                {
                                    self.selected_field = position;
                                    ui.ctx().request_repaint();
                                }
                            });
                            row.col(|ui| {
                                ui.label(format!("0x{:02X}", field.native_type));
                            });
                            row.col(|ui| {
                                ui.label(field.scalar_type.as_deref().unwrap_or("Unsupported"));
                            });
                            row.col(|ui| {
                                ui.label(field.storage_bytes.to_string());
                            });
                        });
                    });
            });
        ui.separator();
        ui.label(RichText::new("Field Properties").strong());
        let Some(field) = schema.fields.get(self.selected_field) else {
            return;
        };
        ScrollArea::vertical().id_salt("field_properties").show(ui, |ui| {
            egui::Grid::new("field_properties_grid").num_columns(2).show(ui, |ui| {
                property(ui, "Field name", &field.name);
                property(ui, "Native field number", field.number.to_string());
                property(ui, "Fixed storage", yes_no(field.fixed));
                property(ui, "Native flags", format!("0x{:02X}", field.flags));
                property(ui, "Extended flags", format!("0x{:02X}", field.extended_flags));
                property(ui, "Reader NULL support", field.reader_nullable.map(yes_no).unwrap_or("Unclassified"));
                if let Some((precision, scale)) = field.decimal_parameters {
                    property(ui, "Precision / scale", format!("{precision} / {scale}"));
                }
                if let Some(id) = field.complex_type_id {
                    property(ui, "Complex type ID", id.to_string());
                }
            });
            ui.small("Reader NULL support describes the read carrier, not the declared Required property.");
        });
    }
}

fn property(ui: &mut Ui, label: &str, value: impl Into<String>) {
    ui.label(label);
    ui.add(egui::Label::new(value.into()).selectable(true));
    ui.end_row();
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Data;
    use crate::workspace::DocumentView;

    fn field(name: &str, number: usize, scalar_type: Option<&str>) -> Field {
        Field {
            name: name.into(),
            number,
            native_type: 0x99,
            storage_bytes: 16,
            fixed: true,
            flags: 3,
            extended_flags: 0,
            scalar_type: scalar_type.map(str::to_owned),
            reader_nullable: None,
            decimal_parameters: None,
            complex_type_id: None,
        }
    }

    fn schema() -> Schema {
        Schema {
            table: "Mixed".into(),
            declared_rows: 42,
            fields: vec![
                field("Original unsupported name", 7, None),
                field("Amount", 2, Some("Int32")),
            ],
        }
    }

    #[test]
    fn design_publication_and_source_reset_preserve_independent_result_slots() {
        let mut state = State::default();
        state.set_table_data(Data {
            source_revision: None,
            table: "Records".into(),
            offset: 100,
            columns: vec![],
            rows: vec![],
            has_more: false,
        });
        state.set_query_data(Data {
            source_revision: None,
            table: "Query result".into(),
            offset: 0,
            columns: vec![],
            rows: vec![],
            has_more: false,
        });
        state.prepare_schema_view();
        state.set_schema(schema());
        state.dismiss_backstage();
        assert!(state.view == DocumentView::Design);
        assert!(!state.query_ready());
        assert!(state.visible_data().is_none());
        state.selected_field = 1;
        assert_eq!(
            state.schema.as_ref().unwrap().fields[state.selected_field].number,
            2
        );
        state.view = DocumentView::Table;
        assert_eq!(state.visible_data().unwrap().offset, 100);
        state.view = DocumentView::Query;
        assert_eq!(state.visible_data().unwrap().table, "Query result");
        state.prepare_schema_view();
        state.set_schema(schema());
        state.dismiss_backstage();
        assert_eq!(state.selected_field, 0);
        state.clear_schema();
        assert!(state.schema.is_none());
        assert!(state.view == DocumentView::Table);
        assert_eq!(state.data.as_ref().unwrap().offset, 100);
    }

    #[test]
    fn late_schema_reply_preserves_the_more_recent_document_choice() {
        let mut state = State::default();
        state.prepare_schema_view();
        state.select_view(DocumentView::Query);
        state.set_schema(schema());
        state.dismiss_backstage();
        assert!(state.view == DocumentView::Query);
        assert_eq!(state.schema.as_ref().unwrap().table, "Mixed");

        state.prepare_schema_view();
        state.set_schema(schema());
        state.dismiss_backstage();
        assert!(state.view == DocumentView::Design);
        state.prepare_schema_view();
        state.select_view(DocumentView::Table);
        state.set_schema(schema());
        state.dismiss_backstage();
        assert!(state.view == DocumentView::Table);

        state.prepare_schema_view();
        state.clear_schema();
        state.set_schema(schema());
        state.dismiss_backstage();
        assert!(state.view == DocumentView::Table);
    }

    #[test]
    fn design_render_is_inert_for_unsupported_and_empty_field_declarations() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        state.prepare_schema_view();
        state.set_schema(schema());
        state.dismiss_backstage();
        for empty in [false, true] {
            if empty {
                state.schema.as_mut().unwrap().fields.clear();
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    assert!(state.show(ui).is_empty());
                },
            );
            assert!(!output.shapes.is_empty());
            output.textures_delta.clear();
        }
        assert!(state.schema.as_ref().unwrap().fields.is_empty());
    }
}
