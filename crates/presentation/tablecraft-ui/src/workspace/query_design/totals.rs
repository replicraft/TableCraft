//! Query-design aggregate roles are draft data, separate from datasheet totals.
use crate::localization::tr;
use crate::workspace::State;
use crate::workspace::focus::{self, Pane};
use egui::Ui;
use tablecraft_builders::query::{DesignColumn, DesignTotal, validate_totals_off};

impl State {
    pub(super) fn query_design_totals_toggle_available(&mut self) -> bool {
        let Some(session) = self.query_design.session.as_mut() else {
            return false;
        };
        if !session.draft.totals {
            return true;
        }
        if let Some((revision, available)) = session.totals_off_qualification
            && revision == session.revision
        {
            return available;
        }
        // Editable text is already bounded by the designer. Parse once per
        // changed draft, never on every Ribbon frame. This cache grants no run
        // authority; the consuming programmatic guard still validates afresh.
        let available = validate_totals_off(&session.draft).is_ok();
        session.totals_off_qualification = Some((session.revision, available));
        available
    }

    pub(super) fn toggle_query_design_totals(&mut self) -> bool {
        if !self.query_design_ready() || !self.query_design_visible() {
            return false;
        }
        let Some(session) = self.query_design.session.as_mut() else {
            return false;
        };
        if session.draft.totals && validate_totals_off(&session.draft).is_err() {
            session.error = Some(self.localizer.text("qbe-totals-off-unavailable"));
            return false;
        }
        let Some(revision) = session.revision.checked_add(1) else {
            session.error = Some(self.localizer.text("qbe-limit"));
            return false;
        };
        session.draft.totals = !session.draft.totals;
        session.revision = revision;
        session.error = None;
        true
    }
}

pub(super) fn column(ui: &mut Ui, column: &mut DesignColumn, index: usize) -> bool {
    let response = egui::ComboBox::from_id_salt("qbe_total")
        .selected_text(tr(ui, key(column.total)))
        .show_ui(ui, |ui| {
            let mut changed = false;
            for role in [
                DesignTotal::GroupBy,
                DesignTotal::Sum,
                DesignTotal::Avg,
                DesignTotal::Min,
                DesignTotal::Max,
                DesignTotal::Count,
                DesignTotal::First,
                DesignTotal::Last,
                DesignTotal::Expression,
                DesignTotal::Where,
            ] {
                let eligible =
                    role != DesignTotal::Where || (!column.show && column.sort.is_none());
                changed |= ui
                    .add_enabled_ui(eligible, |ui| {
                        ui.selectable_value(&mut column.total, role, tr(ui, key(role)))
                    })
                    .inner
                    .on_disabled_hover_text(tr(ui, "qbe-total-where-requires-hidden"))
                    .changed();
            }
            changed
        });
    let label = format!("{} {}", tr(ui, "qbe-total"), index + 1);
    ui.ctx()
        .accesskit_node_builder(response.response.id, |node| node.set_label(label));
    focus::register(
        ui,
        Pane::QueryGrid,
        &response.response,
        response.response.has_focus(),
    );
    response.inner.unwrap_or(false)
}

fn key(role: DesignTotal) -> &'static str {
    match role {
        DesignTotal::GroupBy => "qbe-total-group-by",
        DesignTotal::Sum => "qbe-total-sum",
        DesignTotal::Avg => "qbe-total-avg",
        DesignTotal::Min => "qbe-total-min",
        DesignTotal::Max => "qbe-total-max",
        DesignTotal::Count => "qbe-total-count",
        DesignTotal::First => "qbe-total-first",
        DesignTotal::Last => "qbe-total-last",
        DesignTotal::Expression => "qbe-total-expression",
        DesignTotal::Where => "qbe-total-where",
    }
}

#[cfg(test)]
mod tests;
