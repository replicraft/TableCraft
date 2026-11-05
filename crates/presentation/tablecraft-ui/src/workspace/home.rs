//! Working commands share the dense ribbon with explicitly disabled previews.
use super::{Action, DocumentView, State, ribbon};
use ribbon::{CommandSize, Icon, command, group};

impl State {
    pub(super) fn home(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        ribbon::strip(ui, |ui| {
            group(ui, &t.text("ribbon-views"), |ui| {
                let datasheet_available = match self.view {
                    DocumentView::Table => self.data.is_some(),
                    DocumentView::Design => self.schema.as_ref().is_some_and(|schema| {
                        self.data
                            .as_ref()
                            .is_some_and(|data| data.table == schema.table)
                    }),
                    DocumentView::Query => false,
                };
                if command(
                    ui,
                    Icon::Table,
                    &t.text("ribbon-datasheet"),
                    !self.busy && datasheet_available,
                    CommandSize::Small,
                )
                .clicked()
                {
                    self.select_view(DocumentView::Table);
                }
                let table = match self.view {
                    DocumentView::Table => self.data.as_ref().map(|data| data.table.clone()),
                    DocumentView::Design => self.schema.as_ref().map(|schema| schema.table.clone()),
                    DocumentView::Query => None,
                };
                if command(
                    ui,
                    Icon::TableEdit,
                    &t.text("ribbon-table-design"),
                    !self.busy && table.is_some(),
                    CommandSize::Small,
                )
                .clicked()
                {
                    actions.push(Action::ViewSchema {
                        table: table.expect("enabled for an existing table"),
                    });
                }
            });
            if self.ribbon_tab == ribbon::Tab::TableDesign && self.mock_preview {
                ribbon::design(ui, &t);
                return;
            }
            group(ui, &t.text("ribbon-clipboard"), |ui| {
                ui.horizontal(|ui| {
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &t,
                            Icon::Paste,
                            "ribbon-paste",
                            CommandSize::Large,
                        );
                    }
                    ui.vertical(|ui| {
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::Cut,
                                "ribbon-cut",
                                CommandSize::Small,
                            );
                        }
                        if command(
                            ui,
                            Icon::Copy,
                            &t.text("ribbon-copy"),
                            self.can_copy_selection(),
                            CommandSize::Small,
                        )
                        .on_hover_text(t.text(if self.record_selected() {
                            "ribbon-copy-record-help"
                        } else {
                            "ribbon-copy-cell-help"
                        }))
                        .clicked()
                        {
                            self.copy_selected(ui.ctx());
                        }
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::PaintBrush,
                                "ribbon-format-painter",
                                CommandSize::Small,
                            );
                        }
                    });
                });
            });
            if self.mock_preview {
                ribbon::home(ui, &t);
            }
        });
    }
}

#[cfg(test)]
mod tests;
