//! Object navigation emits page intents without owning a database source.
use super::{Action, DocumentView, State};
use egui::{Color32, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui};

const TABLE_ICON_COLOR: Color32 = Color32::from_rgb(66, 96, 144);

impl State {
    pub(super) fn navigation(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let mut selected_table = false;
        egui::Panel::left("navigation")
            .default_size(210.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(5.0);
                ui.label(RichText::new("All Access Objects").strong().size(15.0));
                ui.add(TextEdit::singleline(&mut self.filter).hint_text("Search tables"));
                ui.checkbox(&mut self.show_system, "Show system tables");
                ui.separator();
                ui.label(RichText::new("Tables").strong());
                if let Some(db) = &self.database {
                    let filter = self.filter.to_lowercase();
                    ScrollArea::vertical().show(ui, |ui| {
                        for table in &db.tables {
                            if (!self.show_system && table.system)
                                || !table.name.to_lowercase().contains(&filter)
                            {
                                continue;
                            }
                            let selected = self
                                .data
                                .as_ref()
                                .is_some_and(|data| data.table == table.name)
                                && self.view == DocumentView::Table;
                            ui.horizontal(|ui| {
                                table_icon(ui);
                                let response = ui
                                    .selectable_label(selected, &table.name)
                                    .on_hover_text(&table.storage);
                                response.context_menu(|ui| {
                                    if ui
                                        .add_enabled(!self.busy, egui::Button::new("View schema"))
                                        .clicked()
                                    {
                                        actions.push(Action::ViewSchema {
                                            table: table.name.clone(),
                                        });
                                        ui.close();
                                    }
                                });
                                if response.clicked() {
                                    selected_table = true;
                                    actions.push(Action::Page {
                                        table: table.name.clone(),
                                        offset: 0,
                                    });
                                }
                                if table.system {
                                    ui.weak("system");
                                }
                            });
                        }
                    });
                } else {
                    ui.weak("Open a database to see its tables.");
                }
            });
        if selected_table {
            self.select_view(DocumentView::Table);
        }
    }
}

// Decorative geometry avoids relying on a table-symbol glyph absent from the
// bundled fonts. The adjacent table-name control supplies the accessible name.
fn table_icon(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
    let rect = rect.shrink(1.0);
    let stroke = Stroke::new(1.0, TABLE_ICON_COLOR);
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
    for fraction in [1.0 / 3.0, 2.0 / 3.0] {
        let y = rect.top() + rect.height() * fraction;
        ui.painter().line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
    }
    ui.painter()
        .line_segment([rect.center_top(), rect.center_bottom()], stroke);
}
