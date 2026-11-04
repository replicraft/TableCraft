use super::Data;
use egui::{RichText, ScrollArea, Ui};
use egui_extras::{Column, TableBuilder};

pub(super) fn show(ui: &mut Ui, data: &Data) {
    ScrollArea::horizontal()
        .id_salt("datasheet_horizontal")
        .show(ui, |ui| {
            TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::exact(58.0))
                .columns(Column::initial(160.0).at_least(80.0), data.columns.len())
                .header(34.0, |mut header| {
                    header.col(|ui| {
                        ui.label(RichText::new("#").strong());
                    });
                    for name in &data.columns {
                        header.col(|ui| {
                            ui.label(RichText::new(name).strong());
                        });
                    }
                })
                .body(|body| {
                    body.rows(30.0, data.rows.len(), |mut row| {
                        let index = row.index();
                        row.col(|ui| {
                            ui.weak((u64::from(data.offset) + index as u64 + 1).to_string());
                        });
                        for cell in &data.rows[index] {
                            row.col(|ui| {
                                ui.add(egui::Label::new(cell).truncate())
                                    .on_hover_text(cell);
                            });
                        }
                    });
                });
        });
}
