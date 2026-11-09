//! Full native-view totals are service results, never arithmetic over a UI page.
mod controls;
mod preferences;
mod rebind_memory;

pub(crate) use controls::Choice;
pub(super) use preferences::TotalsPreferences;

use super::{Action, Cell, Column, DocumentView, ObjectKind, ObjectTab, State};
use crate::{CellKind, DisplayControl};
use tablecraft_display::Options;

const MAX_TOTAL_FIELDS: usize = 32;
const MAX_AGGREGATES: usize = 7;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalsTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalAggregate {
    Count,
    Sum,
    Average,
    Minimum,
    Maximum,
    StandardDeviation,
    Variance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TotalSpec {
    pub column: String,
    pub aggregate: TotalAggregate,
}

pub struct TotalsField {
    pub column: Column,
    pub aggregates: Vec<TotalAggregate>,
}
pub struct TotalsDescription {
    pub target: TotalsTarget,
    pub fields: Vec<TotalsField>,
}
pub struct TotalsData {
    pub target: TotalsTarget,
    pub specs: Vec<TotalSpec>,
    pub cells: Vec<Cell>,
    pub source_rows: u32,
    pub view_rows: u32,
}

pub(crate) struct Row<'a> {
    pub target: &'a TotalsTarget,
    pub fields: Option<&'a [TotalsField]>,
    pub specs: &'a [TotalSpec],
    pub result: Option<&'a TotalsData>,
    pub enabled: bool,
    pub pending: bool,
    pub error: bool,
    pub selected_column: Option<&'a str>,
}

#[derive(Default)]
pub(super) struct Totals {
    session: Option<Box<Session>>,
}

struct Session {
    visible: bool,
    target: Option<TotalsTarget>,
    description: Option<TotalsDescription>,
    specs: Vec<TotalSpec>,
    result: Option<TotalsData>,
    pending: Option<Pending>,
    queued: bool,
    error: Option<String>,
    selected_column: Option<String>,
}

struct Pending {
    request_id: u64,
    object: ObjectTab,
    publication: u64,
    formatting: Options,
    specs: Option<Vec<TotalSpec>>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            visible: true,
            target: None,
            description: None,
            specs: Vec::new(),
            result: None,
            pending: None,
            queued: false,
            error: None,
            selected_column: None,
        }
    }
}

impl Totals {
    pub(in crate::workspace) fn row(
        &self,
        target: &TotalsTarget,
        enabled: bool,
    ) -> Option<Row<'_>> {
        let session = self.session.as_ref()?;
        if !session.visible || session.target.as_ref() != Some(target) {
            return None;
        }
        Some(Row {
            target: session.target.as_ref()?,
            fields: session
                .description
                .as_ref()
                .map(|description| description.fields.as_slice()),
            specs: &session.specs,
            result: session
                .result
                .as_ref()
                .filter(|result| result.specs == session.specs),
            enabled,
            pending: session.pending.is_some() || session.queued,
            error: session.error.is_some(),
            selected_column: session.selected_column.as_deref(),
        })
    }
}

impl State {
    fn current_totals_target(&self) -> Option<TotalsTarget> {
        if self.view != DocumentView::Table {
            return None;
        }
        let data = self.data.as_ref()?;
        let revision = data.source_revision?;
        let object = self.active_object()?;
        if object.target.document_revision != revision
            || !matches!(&object.target.kind, ObjectKind::Table { name } if name == &data.table)
            || self.database.as_ref()?.document_revision != revision
        {
            return None;
        }
        if data.row_ordinals.is_some() && self.active_filter_target().is_none() {
            return None;
        }
        Some(TotalsTarget {
            document_revision: revision,
            table: data.table.clone(),
            filter_id: self.active_filter_target().map(|target| target.filter_id),
            enabled: self.table_filter_enabled(),
        })
    }

    fn totals_controls_ready(&self) -> bool {
        !self.busy
            && !self.edit_dialog_open()
            && !self.zoom_dialog_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && !self.backstage_visible()
            && !self.table_filter_pending()
    }

    pub(in crate::workspace) fn totals_render_context(&self) -> Option<(TotalsTarget, bool)> {
        self.totals_visible().then_some(())?;
        Some((
            self.current_totals_target()?,
            self.totals_controls_ready() && self.totals_request_id().is_none(),
        ))
    }

    pub fn totals_visible(&self) -> bool {
        self.totals
            .session
            .as_ref()
            .is_some_and(|session| session.visible)
    }

    pub fn totals_request_id(&self) -> Option<u64> {
        self.totals
            .session
            .as_ref()?
            .pending
            .as_ref()
            .map(|pending| pending.request_id)
    }

    pub fn total_selection_active(&self) -> bool {
        let Some(session) = &self.totals.session else {
            return false;
        };
        session.visible
            && self.current_totals_target().as_ref() == session.target.as_ref()
            && session.selected_column.as_ref().is_some_and(|column| {
                self.data
                    .as_ref()
                    .is_some_and(|data| data.column_index(column).is_some())
            })
    }

    pub fn selected_total_cell(&self) -> Option<&Cell> {
        if !self.total_selection_active() {
            return None;
        }
        let session = self.totals.session.as_ref()?;
        if session.pending.is_some() || session.queued || session.error.is_some() {
            return None;
        }
        self.totals_cell(session.selected_column.as_deref()?)
    }

    pub(in crate::workspace) fn apply_totals_selection(
        &mut self,
        selection: Option<crate::datasheet::SelectionOwner>,
    ) {
        let Some(selection) = selection else {
            return;
        };
        let column = match selection {
            crate::datasheet::SelectionOwner::Native => None,
            crate::datasheet::SelectionOwner::Total { target, column } => {
                if self.current_totals_target().as_ref() != Some(&target)
                    || !self.totals_visible()
                    || self
                        .data
                        .as_ref()
                        .is_none_or(|data| data.column_index(&column).is_none())
                {
                    return;
                }
                Some(column)
            }
        };
        if let Some(session) = &mut self.totals.session {
            session.selected_column = column;
        }
    }

    pub fn totals_cell(&self, column: &str) -> Option<&Cell> {
        let session = self.totals.session.as_ref()?;
        if !session.visible || self.current_totals_target().as_ref() != session.target.as_ref() {
            return None;
        }
        let result = session.result.as_ref()?;
        if result.specs != session.specs {
            return None;
        }
        let index = result.specs.iter().position(|spec| spec.column == column)?;
        result.cells.get(index)
    }

    pub fn can_describe_totals(&self, target: &TotalsTarget) -> bool {
        self.totals_controls_ready()
            && self.current_totals_target().as_ref() == Some(target)
            && self.totals.session.as_ref().is_some_and(|session| {
                session.visible
                    && session.target.as_ref() == Some(target)
                    && session.description.is_none()
                    && session.pending.is_none()
                    && session.error.is_none()
            })
    }

    pub fn begin_totals_description(&mut self, target: &TotalsTarget, request_id: u64) -> bool {
        if !self.can_describe_totals(target) {
            if let Some(session) = &mut self.totals.session {
                session.queued = false;
            }
            return false;
        }
        let pending = self.new_totals_pending(request_id, None);
        let session = self.totals.session.as_mut().expect("admitted totals row");
        session.pending = Some(pending);
        session.queued = false;
        true
    }

    fn new_totals_pending(&self, request_id: u64, specs: Option<Vec<TotalSpec>>) -> Pending {
        Pending {
            request_id,
            object: self.active_object().expect("native totals object"),
            publication: self.focus_publication,
            formatting: self.formatting.clone(),
            specs,
        }
    }

    fn totals_reply_current(&self, request_id: u64) -> bool {
        let Some(session) = &self.totals.session else {
            return false;
        };
        let Some(pending) = &session.pending else {
            return false;
        };
        pending.request_id == request_id
            && self.active_object().as_ref() == Some(&pending.object)
            && pending.publication == self.focus_publication
            && pending.formatting == self.formatting
            && self.current_totals_target().as_ref() == session.target.as_ref()
    }

    pub fn set_totals_description(
        &mut self,
        request_id: u64,
        description: TotalsDescription,
    ) -> bool {
        if !self.totals_reply_current(request_id) {
            return false;
        }
        let session = self
            .totals
            .session
            .as_ref()
            .expect("current totals request");
        if !session.visible {
            return false;
        }
        let metadata = session
            .pending
            .as_ref()
            .is_some_and(|pending| pending.specs.is_none());
        let columns = &self.data.as_ref().expect("native totals source").columns;
        let valid = metadata
            && session.visible
            && session.target.as_ref() == Some(&description.target)
            && description.fields.len() == columns.len()
            && description
                .fields
                .iter()
                .zip(columns)
                .all(|(field, column)| {
                    field.column.name == column.name
                        && field.aggregates.len() <= MAX_AGGREGATES
                        && field
                            .aggregates
                            .iter()
                            .enumerate()
                            .all(|(index, aggregate)| {
                                !field.aggregates[..index].contains(aggregate)
                            })
                });
        if !valid {
            self.fail_totals(request_id, self.localizer.text("totals-invalid-result"));
            return false;
        }
        let session = self
            .totals
            .session
            .as_mut()
            .expect("current totals request");
        session.pending = None;
        session.description = Some(description);
        if !specs_supported(session, &session.specs) {
            session.error = Some(self.localizer.text("totals-choices-unavailable"));
        }
        true
    }

    pub fn can_compute_totals(&self, target: &TotalsTarget, specs: &[TotalSpec]) -> bool {
        self.totals_controls_ready()
            && self.current_totals_target().as_ref() == Some(target)
            && self.totals.session.as_ref().is_some_and(|session| {
                session.visible
                    && session.target.as_ref() == Some(target)
                    && session.pending.is_none()
                    && session.error.is_none()
                    && specs == session.specs
                    && !specs.is_empty()
                    && specs_supported(session, specs)
            })
    }

    pub fn begin_totals_compute(
        &mut self,
        request_id: u64,
        target: &TotalsTarget,
        specs: Vec<TotalSpec>,
    ) -> bool {
        if !self.can_compute_totals(target, &specs) {
            if let Some(session) = &mut self.totals.session {
                session.queued = false;
            }
            return false;
        }
        let pending = self.new_totals_pending(request_id, Some(specs));
        let session = self
            .totals
            .session
            .as_mut()
            .expect("admitted totals selection");
        session.pending = Some(pending);
        session.queued = false;
        true
    }

    fn totals_counts_match(&self, result: &TotalsData) -> bool {
        if let Some((source, view)) = self.totals_native_view_counts() {
            return result.source_rows == source && result.view_rows == view;
        }
        let Some(data) = &self.data else {
            return false;
        };
        let Some(end) = u64::from(data.offset).checked_add(data.rows.len() as u64) else {
            return false;
        };
        if data.row_ordinals.is_none() && !data.has_more {
            return u64::from(result.source_rows) == end && result.source_rows == result.view_rows;
        }
        end <= u64::from(result.view_rows) && (!data.has_more || end < u64::from(result.view_rows))
    }

    pub fn set_totals_result(&mut self, request_id: u64, mut result: TotalsData) -> bool {
        if !self.totals_reply_current(request_id) {
            return false;
        }
        let session = self
            .totals
            .session
            .as_ref()
            .expect("current totals request");
        if !session.visible {
            return false;
        }
        let valid = session.visible
            && session.target.as_ref() == Some(&result.target)
            && session
                .pending
                .as_ref()
                .and_then(|pending| pending.specs.as_ref())
                == Some(&result.specs)
            && result.specs == session.specs
            && result.cells.len() == result.specs.len()
            && self.totals_counts_match(&result)
            && result.view_rows <= result.source_rows
            && (result.target.enabled || result.view_rows == result.source_rows)
            && result.cells.iter().all(|cell| {
                matches!(
                    cell.kind,
                    CellKind::Scalar {
                        stored_calculation: false,
                        boolean: None,
                        display_control: DisplayControl::Uninspected | DisplayControl::Absent,
                        ..
                    }
                )
            });
        if !valid {
            self.fail_totals(request_id, self.localizer.text("totals-invalid-result"));
            return false;
        }
        for cell in &mut result.cells {
            self.format_cache.apply(cell);
        }
        let session = self
            .totals
            .session
            .as_mut()
            .expect("current totals request");
        session.result = Some(result);
        session.pending = None;
        session.error = None;
        true
    }

    pub fn fail_totals(&mut self, request_id: u64, message: String) {
        if !self.totals_reply_current(request_id) {
            return;
        }
        let session = self
            .totals
            .session
            .as_mut()
            .expect("current totals request");
        session.pending = None;
        session.queued = false;
        session.error = Some(message);
    }

    pub fn cancel_totals(&mut self, request_id: u64) -> bool {
        if self.totals_request_id() != Some(request_id) {
            return false;
        }
        let session = self
            .totals
            .session
            .as_mut()
            .expect("current totals request");
        session.pending = None;
        session.queued = false;
        if session.visible {
            session.error = Some(self.localizer.text("totals-cancelled"));
        }
        true
    }

    pub fn reject_totals_request(&mut self, target: &TotalsTarget, message: String) {
        if self.current_totals_target().as_ref() != Some(target) {
            return;
        }
        if let Some(session) = &mut self.totals.session
            && session.target.as_ref() == Some(target)
            && session.pending.is_none()
        {
            session.queued = false;
            session.error = Some(message);
        }
    }

    pub fn take_totals_request(&mut self) -> Option<Action> {
        self.validate_totals();
        let target = self.current_totals_target()?;
        let session = self.totals.session.as_ref()?;
        if !session.visible
            || session.pending.is_some()
            || session.queued
            || session.error.is_some()
            || !self.totals_controls_ready()
        {
            return None;
        }
        let action = if session.description.is_none() {
            Action::DescribeTotals { target }
        } else if !session.specs.is_empty()
            && session
                .result
                .as_ref()
                .is_none_or(|result| result.specs != session.specs)
        {
            Action::ComputeTotals {
                target,
                specs: session.specs.clone(),
            }
        } else {
            return None;
        };
        self.totals
            .session
            .as_mut()
            .expect("visible totals row")
            .queued = true;
        Some(action)
    }

    pub(super) fn validate_totals(&mut self) {
        let target = self.current_totals_target();
        let Some(session) = &mut self.totals.session else {
            return;
        };
        if self.view != DocumentView::Table {
            return;
        }
        if target != session.target {
            session.target = target;
            session.description = None;
            session.result = None;
            session.pending = None;
            session.queued = false;
            session.error = None;
        } else if session.pending.as_ref().is_some_and(|pending| {
            pending.publication != self.focus_publication || pending.formatting != self.formatting
        }) {
            session.pending = None;
            session.queued = false;
        }
    }

    pub(super) fn refresh_totals_display(&mut self) {
        if let Some(result) = self
            .totals
            .session
            .as_mut()
            .and_then(|session| session.result.as_mut())
        {
            for cell in &mut result.cells {
                self.format_cache.apply(cell);
            }
        }
    }
}

fn specs_supported(session: &Session, specs: &[TotalSpec]) -> bool {
    let Some(description) = &session.description else {
        return false;
    };
    specs.len() <= MAX_TOTAL_FIELDS
        && specs.iter().enumerate().all(|(index, spec)| {
            !specs[..index]
                .iter()
                .any(|previous| previous.column == spec.column)
                && description.fields.iter().any(|field| {
                    field.column.name == spec.column && field.aggregates.contains(&spec.aggregate)
                })
        })
}

#[cfg(test)]
mod tests;
