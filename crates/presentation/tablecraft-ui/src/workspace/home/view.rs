//! The existing table views share the Access-style large View dropdown.
use super::{Action, CommandSize, DocumentView, Icon, State, command};
use egui::{Popup, Stroke, accesskit::HasPopup};

const VIEW_ARROW_HALF_WIDTH: f32 = 3.0;
const VIEW_ARROW_BOTTOM_PADDING: f32 = 6.0;

impl State {
    pub(super) fn view_menu(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        let datasheet_available = match self.view {
            DocumentView::Table => self.data.is_some(),
            DocumentView::Design => self.schema.as_ref().is_some_and(|schema| {
                self.data
                    .as_ref()
                    .is_some_and(|data| data.table == schema.table)
            }),
            DocumentView::Query => false,
        };
        let table = match self.view {
            DocumentView::Table => self.data.as_ref().map(|data| data.table.clone()),
            DocumentView::Design => self.schema.as_ref().map(|schema| schema.table.clone()),
            DocumentView::Query => None,
        };
        let enabled = !self.busy && table.is_some();
        let icon = if self.view == DocumentView::Design {
            Icon::Table
        } else {
            Icon::TableEdit
        };
        let response = command(
            ui,
            icon,
            &t.text("ribbon-view"),
            enabled,
            CommandSize::Large,
        );
        let colors = crate::theme::palette(ui.ctx());
        let color = if response.enabled() {
            colors.ink
        } else {
            colors.muted
        };
        let center = egui::pos2(
            response.rect.center().x,
            response.rect.bottom() - VIEW_ARROW_BOTTOM_PADDING,
        );
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-VIEW_ARROW_HALF_WIDTH, -VIEW_ARROW_HALF_WIDTH),
                center + egui::vec2(VIEW_ARROW_HALF_WIDTH, -VIEW_ARROW_HALF_WIDTH),
                center,
            ],
            color,
            Stroke::NONE,
        ));

        if !response.enabled() {
            Popup::close_id(ui.ctx(), Popup::default_response_id(&response));
        }
        if response.enabled() {
            Popup::menu(&response).show(|ui| {
                if command(
                    ui,
                    Icon::Table,
                    &t.text("ribbon-datasheet"),
                    datasheet_available,
                    CommandSize::Small,
                )
                .clicked()
                {
                    self.select_view(DocumentView::Table);
                    ui.close();
                }
                if command(
                    ui,
                    Icon::TableEdit,
                    &t.text("ribbon-table-design"),
                    table.is_some(),
                    CommandSize::Small,
                )
                .clicked()
                {
                    actions.push(Action::ViewSchema {
                        table: table.expect("enabled for an existing table"),
                    });
                    ui.close();
                }
            });
        }
        let expanded = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(expanded);
        });
    }
}
