//! F6 chooses only enabled anchors published during the current workspace frame.
use super::{DocumentView, State};
use egui::{Context, Event, Id, Key, Modifiers, Popup, Response, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pane {
    QuerySources,
    QueryGrid,
    Document,
    Results,
    Properties,
    Navigation,
    Ribbon,
}

impl Pane {
    const ORDER: [Self; 7] = [
        Self::QuerySources,
        Self::QueryGrid,
        Self::Document,
        Self::Results,
        Self::Properties,
        Self::Navigation,
        Self::Ribbon,
    ];
}

#[derive(Clone, Copy)]
enum Direction {
    Next,
    Previous,
}

#[derive(Clone)]
struct Anchor {
    pane: Pane,
    id: Id,
    preferred: bool,
    record: bool,
}

#[derive(Clone)]
struct Frame {
    panes: Vec<Pane>,
    number: u64,
    pass: u64,
    scope: Id,
    direction: Option<Direction>,
    anchors: Vec<Anchor>,
}

pub(super) struct Entry {
    scope: Id,
    pane: Pane,
    // Comparison only: a newer user focus choice supersedes deferred entry.
    // This identifier is never used as a future focus target.
    focus_witness: Option<Id>,
}

/// Declare a currently loaded, selected pane whose virtual row needs revealing.
/// This never registers or requests a remembered widget ID.
pub(crate) fn declare(ui: &Ui, pane: Pane) {
    if !ui.is_enabled() {
        return;
    }
    let number = ui.ctx().cumulative_frame_nr();
    ui.ctx().data_mut(|data| {
        let slot = data.get_temp_mut_or_default::<Option<Frame>>(registry_id());
        if let Some(frame) = slot.as_mut()
            && frame.number == number
            && !frame.panes.contains(&pane)
        {
            frame.panes.push(pane);
        }
    });
}

fn registry_id() -> Id {
    Id::new("workspace_pane_focus")
}

/// Standalone widgets register nothing unless a workspace frame is active.
pub(crate) fn register(ui: &Ui, pane: Pane, response: &Response, preferred: bool) {
    register_anchor(ui, pane, response, preferred, false);
}

/// A rendered row/cell control, distinct from an object tab in the same pane.
pub(crate) fn register_record(ui: &Ui, pane: Pane, response: &Response, preferred: bool) {
    register_anchor(ui, pane, response, preferred, true);
}

fn register_anchor(ui: &Ui, pane: Pane, response: &Response, preferred: bool, record: bool) {
    if !response.enabled() || !response.sense.is_focusable() {
        return;
    }
    let number = ui.ctx().cumulative_frame_nr();
    ui.ctx().data_mut(|data| {
        let slot = data.get_temp_mut_or_default::<Option<Frame>>(registry_id());
        if let Some(frame) = slot.as_mut()
            && frame.number == number
        {
            if !frame.panes.contains(&pane) {
                frame.panes.push(pane);
            }
            if let Some(anchor) = frame
                .anchors
                .iter_mut()
                .find(|anchor| anchor.id == response.id)
            {
                anchor.preferred |= preferred;
                anchor.record |= record;
            } else {
                frame.anchors.push(Anchor {
                    pane,
                    id: response.id,
                    preferred,
                    record,
                });
            }
        }
    });
}

impl State {
    pub(super) fn pane_scope(&self) -> Id {
        let document = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let (view, object, source, offset) = match self.view {
            DocumentView::Table => (
                "table",
                self.data.as_ref().map(|data| data.table.as_str()),
                self.data.as_ref().and_then(|data| data.source_revision),
                self.data.as_ref().map(|data| data.offset),
            ),
            DocumentView::Design => (
                "design",
                self.schema.as_ref().map(|schema| schema.table.as_str()),
                None,
                None,
            ),
            DocumentView::Query => {
                let data = if self.saved_query.active {
                    self.saved_query.data.as_ref()
                } else {
                    self.query_data.as_ref()
                };
                (
                    "query",
                    None,
                    data.and_then(|data| data.source_revision),
                    data.map(|data| data.offset),
                )
            }
        };
        let saved = self
            .saved_query
            .active
            .then_some(self.saved_query.target.as_ref())
            .flatten()
            .map(|target| (target.document_revision, target.object_id));
        Id::new((
            "workspace_panes",
            document,
            view,
            object,
            source,
            offset,
            saved,
            self.saved_query.active,
            self.focus_publication,
            self.active_object().map(|object| object.id),
            (
                self.query_design_visible(),
                self.query_design_results_visible(),
            ),
        ))
    }

    pub(super) fn begin_pane_focus(&mut self, ui: &Ui, modal: bool) {
        ui.ctx()
            .data_mut(|data| data.remove::<Option<Frame>>(registry_id()));
        if !ui.is_enabled() || modal || self.backstage_visible() || Popup::is_any_open(ui.ctx()) {
            self.pane_focus_entry = None;
            return;
        }
        let direction = ui.input_mut(|input| {
            let mut direction = None;
            input.events.retain(|event| {
                if let Event::Key {
                    key: Key::F6,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    if *modifiers == Modifiers::NONE {
                        direction = Some(Direction::Next);
                        return false;
                    }
                    if *modifiers == Modifiers::SHIFT {
                        direction = Some(Direction::Previous);
                        return false;
                    }
                }
                true
            });
            direction
        });
        let frame = Frame {
            number: ui.ctx().cumulative_frame_nr(),
            pass: ui.ctx().cumulative_pass_nr(),
            scope: self.pane_scope(),
            direction,
            anchors: Vec::new(),
            panes: Vec::new(),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(registry_id(), Some(frame)));
    }

    pub(super) fn native_record_has_focus(&self, ui: &Ui) -> bool {
        let focused = ui.memory(|memory| memory.focused());
        let number = ui.ctx().cumulative_frame_nr();
        let pass = ui.ctx().cumulative_pass_nr();
        let scope = self.pane_scope();
        ui.ctx().data(|data| {
            data.get_temp::<Option<Frame>>(registry_id())
                .flatten()
                .is_some_and(|frame| {
                    frame.number == number
                        && frame.pass == pass
                        && frame.scope == scope
                        && frame.direction.is_none()
                        && frame.anchors.iter().any(|anchor| {
                            anchor.pane == Pane::Document
                                && anchor.record
                                && Some(anchor.id) == focused
                        })
                })
        })
    }

    pub(super) fn finish_pane_focus(&mut self, ui: &Ui) {
        let frame = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<Option<Frame>>(registry_id()))
            .flatten();
        let Some(frame) = frame else {
            return;
        };
        let Some(direction) = frame.direction else {
            return;
        };
        if frame.scope != self.pane_scope()
            || self.backstage_visible()
            || self.edit_dialog_open()
            || self.contents_request_id().is_some()
            || Popup::is_any_open(ui.ctx())
        {
            return;
        }
        if let Some(pane) = cycle(ui.ctx(), &frame, direction) {
            self.pane_focus_entry = Some(Entry {
                scope: frame.scope,
                pane,
                focus_witness: ui.ctx().memory(|memory| memory.focused()),
            });
        }
    }

    pub(super) fn return_to_document_from_ribbon(&mut self, ui: &Ui) {
        let pane = if self.query_design_visible() {
            Pane::QueryGrid
        } else if self.query_design_results_visible() {
            Pane::Results
        } else {
            Pane::Document
        };
        let frame = ui
            .ctx()
            .data(|data| data.get_temp::<Option<Frame>>(registry_id()))
            .flatten();
        let Some(frame) = frame else {
            return;
        };
        if frame.number != ui.ctx().cumulative_frame_nr() || frame.scope != self.pane_scope() {
            return;
        }
        let anchor = frame
            .anchors
            .iter()
            .rev()
            .find(|anchor| anchor.pane == pane && anchor.preferred)
            .or_else(|| frame.anchors.iter().find(|anchor| anchor.pane == pane));
        if let Some(anchor) = anchor {
            ui.ctx()
                .memory_mut(|memory| memory.request_focus(anchor.id));
        } else if let Some(focused) = ui.ctx().memory(|memory| memory.focused()) {
            ui.ctx()
                .memory_mut(|memory| memory.surrender_focus(focused));
        }
        if frame.panes.contains(&pane) {
            self.pane_focus_entry = Some(Entry {
                scope: frame.scope,
                pane,
                focus_witness: ui.ctx().memory(|memory| memory.focused()),
            });
            ui.ctx().request_repaint();
        }
    }

    pub(super) fn bump_focus_publication(&mut self) {
        self.clear_query_parameter_acquisition();
        self.close_query_update();
        self.focus_publication = self.focus_publication.wrapping_add(1);
        self.pane_focus_entry = None;
        self.design_focus_pending = None;
    }

    pub(super) fn take_pane_entry(&mut self, context: &Context) -> Option<Pane> {
        let entry = self.pane_focus_entry.take()?;
        let unchanged_focus = context.memory(|memory| memory.focused()) == entry.focus_witness;
        (entry.scope == self.pane_scope() && unchanged_focus).then_some(entry.pane)
    }
}

fn cycle(context: &Context, frame: &Frame, direction: Direction) -> Option<Pane> {
    let anchors = &frame.anchors;
    let query_design =
        frame.panes.contains(&Pane::QuerySources) || frame.panes.contains(&Pane::QueryGrid);
    // Object tabs also publish Document anchors. In Query Design they must not
    // become an extra stop between the two working panes and the app chrome.
    let panes: Vec<_> = Pane::ORDER
        .into_iter()
        .filter(|pane| frame.panes.contains(pane) && (!query_design || *pane != Pane::Document))
        .collect();
    if panes.is_empty() {
        return None;
    }
    let focused = context.memory(|memory| memory.focused());
    let current = anchors
        .iter()
        .find(|anchor| Some(anchor.id) == focused)
        .map(|anchor| anchor.pane);
    let index = current.and_then(|pane| panes.iter().position(|candidate| *candidate == pane));
    let next = match (index, direction) {
        (Some(index), Direction::Next) => (index + 1) % panes.len(),
        (Some(index), Direction::Previous) => (index + panes.len() - 1) % panes.len(),
        (None, Direction::Next) => 0,
        (None, Direction::Previous) => panes.len() - 1,
    };
    let pane = panes[next];
    let target = anchors
        .iter()
        .rev()
        .find(|anchor| anchor.pane == pane && anchor.preferred)
        .or_else(|| anchors.iter().find(|anchor| anchor.pane == pane));
    if let Some(target) = target {
        context.memory_mut(|memory| memory.request_focus(target.id));
    }
    context.request_repaint();
    Some(pane)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod query_design_tests;
