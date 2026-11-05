//! F6 chooses only enabled anchors published during the current workspace frame.
use super::{DocumentView, State};
use egui::{Context, Event, Id, Key, Modifiers, Popup, Response, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pane {
    Document,
    Results,
    Properties,
    Navigation,
    Ribbon,
}

impl Pane {
    const ORDER: [Self; 5] = [
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
}

#[derive(Clone)]
struct Frame {
    panes: Vec<Pane>,
    number: u64,
    scope: Id,
    direction: Option<Direction>,
    anchors: Vec<Anchor>,
}

pub(super) struct Entry {
    scope: Id,
    pane: Pane,
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
            } else {
                frame.anchors.push(Anchor {
                    pane,
                    id: response.id,
                    preferred,
                });
            }
        }
    });
}

impl State {
    fn pane_scope(&self) -> Id {
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
            scope: self.pane_scope(),
            direction,
            anchors: Vec::new(),
            panes: Vec::new(),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(registry_id(), Some(frame)));
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
            });
        }
    }

    pub(super) fn bump_focus_publication(&mut self) {
        self.focus_publication = self.focus_publication.wrapping_add(1);
        self.pane_focus_entry = None;
        self.design_focus_pending = None;
    }

    pub(super) fn take_pane_entry(&mut self) -> Option<Pane> {
        let entry = self.pane_focus_entry.take()?;
        (entry.scope == self.pane_scope()).then_some(entry.pane)
    }
}

fn cycle(context: &Context, frame: &Frame, direction: Direction) -> Option<Pane> {
    let anchors = &frame.anchors;
    let panes: Vec<_> = Pane::ORDER
        .into_iter()
        .filter(|pane| frame.panes.contains(pane))
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
