//! Source-local roving focus operates only on rows rendered in the current frame.
use super::SourceEvents;
use crate::workspace::focus::{self, Pane};
use egui::{Event, EventFilter, Id, Key, Modifiers, PointerButton, Popup, Response, Ui};
use tablecraft_builders::query::{DesignExpression, DesignSource, DesignTableMetadata};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Output {
    Show,
    Hide,
    Toggle,
}

pub(super) fn same_projection(left: &DesignExpression, right: &DesignExpression) -> bool {
    match (left, right) {
        (DesignExpression::Field(left), DesignExpression::Field(right)) => {
            left.source_id == right.source_id && left.column.eq_ignore_ascii_case(&right.column)
        }
        (DesignExpression::AllFields(left), DesignExpression::AllFields(right)) => left == right,
        _ => false,
    }
}

pub(super) struct List {
    id: Id,
    selected: usize,
    rows: Vec<(Response, DesignExpression)>,
    source_id: u64,
}

impl List {
    pub(super) fn load(
        ui: &Ui,
        source: &DesignSource,
        metadata: Option<&DesignTableMetadata>,
    ) -> Self {
        let mut id = ui.make_persistent_id(("qbe_source_selection", source.id, &source.table));
        if let Some(metadata) = metadata {
            id = id.with(metadata.fields.len());
            for field in &metadata.fields {
                id = id.with(&field.name);
            }
        }
        let selected = ui.data(|data| data.get_temp::<usize>(id)).unwrap_or(0);
        Self {
            id,
            selected,
            rows: Vec::new(),
            source_id: source.id,
        }
    }

    pub(super) fn selected(&self, index: usize) -> bool {
        self.selected == index
    }

    pub(super) fn row(&mut self, response: Response, expression: DesignExpression) {
        if response.enabled() && response.clicked_by(PointerButton::Primary) {
            self.selected = self.rows.len();
            response.request_focus();
        }
        self.rows.push((response, expression));
    }

    pub(super) fn finish(mut self, ui: &Ui, events: &mut SourceEvents) {
        let Some((response, _)) = self.rows.get(self.selected) else {
            return;
        };
        if !ui.is_enabled() || Popup::is_any_open(ui.ctx()) {
            return;
        }
        if response.has_focus() {
            events.selected_source = Some(self.source_id);
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    EventFilter {
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
            let (position, outputs) = ui.input_mut(|input| {
                let mut position = self.selected;
                let mut outputs = Vec::new();
                input.events.retain(|event| {
                    let Event::Key {
                        key,
                        pressed: true,
                        repeat,
                        modifiers,
                        ..
                    } = event
                    else {
                        return true;
                    };
                    if *modifiers == Modifiers::NONE {
                        match key {
                            Key::ArrowDown => position = (position + 1).min(self.rows.len() - 1),
                            Key::ArrowUp => position = position.saturating_sub(1),
                            Key::Home => position = 0,
                            Key::End => position = self.rows.len() - 1,
                            Key::Space => {
                                if !repeat {
                                    outputs.push((position, Output::Toggle));
                                }
                            }
                            Key::Plus => {
                                if !repeat {
                                    outputs.push((position, Output::Show));
                                }
                            }
                            Key::Minus => {
                                if !repeat {
                                    outputs.push((position, Output::Hide));
                                }
                            }
                            _ => return true,
                        }
                        return false;
                    }
                    if *key == Key::Plus && *modifiers == Modifiers::SHIFT {
                        if !repeat {
                            outputs.push((position, Output::Show));
                        }
                        return false;
                    }
                    true
                });
                (position, outputs)
            });
            let moved = self.selected != position;
            self.selected = position;
            let (response, _) = &self.rows[position];
            if moved {
                response.request_focus();
                // request_focus creates a fresh focus record. Install the list
                // filter on that record now, before the next input frame.
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        EventFilter {
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
            }
            if moved || response.gained_focus() {
                response.scroll_to_me(None);
            }
            for (position, output) in outputs {
                // NO_CLONE: the retained command or exact-owner witness needs its own captured
                // identity after this render/dispatch; keep the original catalog,
                // input or owner available for the later checks in this operation.
                events.outputs.push((self.rows[position].1.clone(), output));
                events.pair = None;
            }
        }
        let (selected, _) = &self.rows[self.selected];
        focus::register(
            ui,
            Pane::QuerySources,
            selected,
            events.selected_source == Some(self.source_id),
        );
        ui.data_mut(|data| data.insert_temp(self.id, self.selected));
    }
}

#[cfg(test)]
mod tests;
