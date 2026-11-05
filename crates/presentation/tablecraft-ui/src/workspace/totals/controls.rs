//! The Ribbon toggles visibility; aggregate choices remain local until submitted.
use super::{MAX_TOTAL_FIELDS, Session, TotalAggregate, TotalSpec, TotalsTarget};
use crate::localization::tr;
use crate::workspace::{
    Action, State, diagnostics,
    ribbon::{CommandSize, Icon, command},
};
use egui::{Ui, WidgetInfo, WidgetType};

pub(crate) struct Choice {
    pub target: TotalsTarget,
    pub column: String,
    pub aggregate: Option<TotalAggregate>,
}

impl TotalAggregate {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Count => "totals-count",
            Self::Sum => "totals-sum",
            Self::Average => "totals-average",
            Self::Minimum => "totals-minimum",
            Self::Maximum => "totals-maximum",
            Self::StandardDeviation => "totals-standard-deviation",
            Self::Variance => "totals-variance",
        }
    }
}

impl State {
    pub(in crate::workspace) fn totals_command(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let pending = self.totals_request_id();
        let enabled = self.current_totals_target().is_some()
            && (self.totals_controls_ready() || pending.is_some() && self.busy)
            && !self.edit_dialog_open()
            && !self.backstage_visible();
        let selected = self.totals_visible();
        let label = tr(ui, "ribbon-totals");
        let response = command(ui, Icon::Sum, &label, enabled, CommandSize::Small)
            .on_hover_text(tr(ui, "totals-help"));
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Button, response.enabled(), selected, &label)
        });
        if selected {
            ui.painter().rect_stroke(
                response.rect,
                2.0,
                egui::Stroke::new(1.0, crate::theme::palette(ui.ctx()).accent),
                egui::StrokeKind::Inside,
            );
        }
        if response.clicked()
            && let Some(action) = self.toggle_totals_visibility()
        {
            actions.push(action);
        }
    }

    pub(in crate::workspace) fn toggle_totals_visibility(&mut self) -> Option<Action> {
        self.current_totals_target()?;
        let pending = self.totals_request_id();
        if !self.totals_controls_ready() && !(pending.is_some() && self.busy) {
            return None;
        }
        let mut action = None;
        if let Some(session) = &mut self.totals.session {
            session.visible = !session.visible;
            if !session.visible {
                session.selected_column = None;
                if session.error.is_some() || pending.is_some() {
                    session.result = None;
                }
            }
            session.queued = false;
            session.error = None;
            if !session.visible
                && let Some(request_id) = pending
            {
                action = Some(Action::CancelTotals { request_id });
            }
        } else {
            self.totals.session = Some(Box::new(Session::default()));
        }
        self.validate_totals();
        action
    }

    pub(in crate::workspace) fn apply_total_choices(&mut self, choices: Vec<Choice>) {
        for choice in choices {
            if !self.totals_controls_ready()
                || self.current_totals_target().as_ref() != Some(&choice.target)
            {
                continue;
            }
            let Some(session) = &mut self.totals.session else {
                continue;
            };
            if !session.visible || session.pending.is_some() {
                continue;
            }
            let supported = choice.aggregate.is_none_or(|aggregate| {
                session.description.as_ref().is_some_and(|description| {
                    description.fields.iter().any(|field| {
                        field.column.name == choice.column && field.aggregates.contains(&aggregate)
                    })
                })
            });
            if !supported {
                continue;
            }
            let existing = session
                .specs
                .iter()
                .position(|spec| spec.column == choice.column);
            match (existing, choice.aggregate) {
                (Some(index), Some(aggregate)) => session.specs[index].aggregate = aggregate,
                (Some(index), None) => {
                    session.specs.remove(index);
                }
                (None, Some(aggregate)) if session.specs.len() < MAX_TOTAL_FIELDS => {
                    session.specs.push(TotalSpec {
                        column: choice.column,
                        aggregate,
                    });
                }
                (None, Some(_)) => {
                    session.error = Some(self.localizer.text("totals-limit"));
                    session.queued = false;
                    continue;
                }
                _ => continue,
            }
            session.result = None;
            session.error = None;
            session.queued = false;
        }
    }

    pub(in crate::workspace) fn totals_report(&self, ui: &mut Ui) {
        if !self.totals_visible() {
            return;
        }
        let Some(session) = &self.totals.session else {
            return;
        };
        if self.current_totals_target().as_ref() != session.target.as_ref() {
            return;
        }
        if self.mock_preview && session.error.is_none() {
            ui.small(tr(ui, "totals-mock-unavailable"));
        }
        if let Some(error) = &session.error {
            ui.small(tr(ui, "totals-error-help"));
            diagnostics::show(ui, ui.make_persistent_id("totals_error"), error);
        }
    }
}
