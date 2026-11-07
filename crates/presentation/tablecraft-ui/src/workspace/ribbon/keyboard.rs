//! Ribbon navigation resolves focus targets only from the current rendered pass.
use super::Tab;
use crate::workspace::State;
use egui::{Context, Event, Id, ImeEvent, Key, LayerId, Modifiers, Popup, Response, Ui};
use std::mem;

#[derive(Default)]
pub(in crate::workspace) struct Keyboard {
    witness: Option<Witness>,
    pending: Option<Entry>,
    reveal: Option<Reveal>,
    composition: Option<Id>,
    handled_frame: Option<u64>,
}

struct Witness {
    scope: Id,
    owner: Id,
    commands: bool,
    frame: u64,
}

struct Reveal {
    scope: Id,
    owner: Id,
}

struct Entry {
    scope: Id,
    focus_witness: Option<Id>,
    display: (bool, bool),
}

#[derive(Clone, Copy)]
enum Intent {
    Enter,
    Tab(bool),
    Commands,
    Command(bool),
    Group(bool),
    Back,
}

#[derive(Clone)]
struct TabAnchor {
    tab: Option<Tab>,
    response: Response,
}

#[derive(Clone)]
struct Group {
    layer: LayerId,
    visible: bool,
    commands: Vec<Response>,
}

#[derive(Clone)]
struct Frame {
    pass: u64,
    scope: Id,
    tab: Tab,
    intents: Vec<Intent>,
    origin: Option<Id>,
    retry: bool,
    tabs: Vec<TabAnchor>,
    groups: Vec<Group>,
    active_group: Option<usize>,
    reveal: Option<Id>,
}

fn registry() -> Id {
    Id::new("workspace_ribbon_keyboard")
}

fn with_frame<T>(context: &Context, operation: impl FnOnce(&mut Frame) -> T) -> Option<T> {
    let pass = context.cumulative_pass_nr();
    context.data_mut(|data| {
        data.get_temp_mut_or_default::<Option<Frame>>(registry())
            .as_mut()
            .filter(|frame| frame.pass == pass)
            .map(operation)
    })
}

fn lock(response: &Response) {
    if response.has_focus() {
        response.ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
    }
}

fn focus(response: &Response) {
    response.request_focus();
    lock(response);
    response.scroll_to_me(None);
    response.ctx.request_repaint();
}

pub(in crate::workspace) fn tab(ui: &Ui, response: &Response, tab: Option<Tab>) {
    if ui.is_visible() && response.enabled() && response.sense.is_focusable() {
        let reveal = with_frame(ui.ctx(), |frame| {
            frame.tabs.push(TabAnchor {
                tab,
                response: response.clone(),
            });
            frame.reveal == Some(response.id)
        })
        .unwrap_or(false);
        reveal_current(response, reveal);
        lock(response);
    }
}

pub(super) fn begin_group(ui: &Ui) -> Option<usize> {
    let layer = ui.layer_id();
    let visible = ui.is_visible() && ui.is_enabled();
    with_frame(ui.ctx(), |frame| {
        let previous = frame.active_group;
        frame.active_group = Some(frame.groups.len());
        frame.groups.push(Group {
            layer,
            visible,
            commands: Vec::new(),
        });
        previous
    })
    .flatten()
}

pub(super) fn end_group(ui: &Ui, previous: Option<usize>) {
    with_frame(ui.ctx(), |frame| frame.active_group = previous);
}

pub(super) fn command(ui: &Ui, response: &Response) {
    if !response.enabled() || !response.sense.is_focusable() || !ui.is_visible() {
        return;
    }
    let registered = with_frame(ui.ctx(), |frame| {
        let group = frame.groups.get_mut(frame.active_group?)?;
        if !group.visible || group.layer != response.layer_id {
            return None;
        }
        group.commands.push(response.clone());
        Some(frame.reveal == Some(response.id))
    })
    .flatten();
    if let Some(reveal) = registered {
        reveal_current(response, reveal);
        lock(response);
    }
}

fn reveal_current(response: &Response, reveal: bool) {
    if reveal && response.has_focus() {
        response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
    }
}

fn key_intent(key: Key, modifiers: Modifiers, commands: Option<bool>) -> Option<Intent> {
    match (key, modifiers, commands) {
        (Key::F10, Modifiers::NONE, _) => Some(Intent::Enter),
        (Key::ArrowLeft, Modifiers::NONE, Some(false)) => Some(Intent::Tab(false)),
        (Key::ArrowRight, Modifiers::NONE, Some(false)) => Some(Intent::Tab(true)),
        (Key::ArrowDown, Modifiers::NONE, Some(false)) => Some(Intent::Commands),
        (Key::Tab, Modifiers::NONE, Some(true)) => Some(Intent::Command(true)),
        (Key::Tab, Modifiers::SHIFT, Some(true)) => Some(Intent::Command(false)),
        (Key::ArrowLeft, Modifiers::CTRL, Some(true)) => Some(Intent::Group(false)),
        (Key::ArrowRight, Modifiers::CTRL, Some(true)) => Some(Intent::Group(true)),
        (Key::Escape, Modifiers::NONE, Some(_)) => Some(Intent::Back),
        _ => None,
    }
}

impl State {
    fn ribbon_keyboard_scope(&self) -> Id {
        self.pane_scope()
            .with((self.ribbon_tab, self.localizer.locale().as_str()))
    }

    fn ribbon_keyboard_available(&self, ui: &Ui) -> bool {
        ui.is_enabled()
            && !self.backstage_visible()
            && !self.find_window_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && !self.zoom_dialog_open()
    }

    pub(in crate::workspace) fn begin_ribbon_keyboard(&mut self, ui: &Ui, modal: bool) {
        let scope = self.ribbon_keyboard_scope();
        let focused = ui.ctx().memory(|memory| memory.focused());
        if self.ribbon_keyboard.composition != focused {
            self.ribbon_keyboard.composition = None;
        }
        let ime = ui.input(|input| {
            let mut ime = false;
            for event in &input.events {
                if let Event::Ime(event) = event {
                    ime = true;
                    // Older host adapters can still send deprecated teardown
                    // events; they must release a retained composition owner.
                    #[allow(deprecated)]
                    match event {
                        ImeEvent::Preedit { text, .. } => {
                            self.ribbon_keyboard.composition =
                                (!text.is_empty()).then_some(focused).flatten();
                        }
                        ImeEvent::Commit(_) | ImeEvent::Disabled => {
                            self.ribbon_keyboard.composition = None;
                        }
                        _ => {}
                    }
                }
            }
            ime
        });
        let frame_number = ui.ctx().cumulative_frame_nr();
        let owner = self.ribbon_keyboard.witness.as_ref().filter(|witness| {
            witness.scope == scope
                && Some(witness.owner) == focused
                && frame_number.saturating_sub(witness.frame) <= 1
        });
        let commands = owner.map(|witness| witness.commands);
        let allowed = !modal
            && self.ribbon_keyboard_available(ui)
            && !Popup::is_any_open(ui.ctx())
            && !ime
            && self.ribbon_keyboard.composition.is_none()
            && !ui.input(|input| {
                input.events.iter().any(|event| {
                    matches!(event, Event::Key { key: Key::F6, pressed: true, modifiers, .. }
                    if *modifiers == Modifiers::NONE || *modifiers == Modifiers::SHIFT)
                })
            });
        let mut intents = Vec::new();
        if allowed {
            ui.input_mut(|input| {
                input.events.retain(|event| {
                    if let Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = event
                        && let Some(intent) = key_intent(*key, *modifiers, commands)
                    {
                        intents.push(intent);
                        return false;
                    }
                    true
                })
            });
        }
        let mut retry = false;
        if self.ribbon_keyboard.handled_frame == Some(frame_number) {
            intents.clear();
        } else if intents.is_empty() && allowed {
            if let Some(entry) = self.ribbon_keyboard.pending.take()
                && entry.scope == scope
                && entry.focus_witness == focused
                && commands == Some(false)
                && entry.display == (self.ribbon_collapsed(), self.ribbon_temporarily_open())
                && !ui.input(|input| input.pointer.any_pressed())
            {
                intents.push(Intent::Commands);
                retry = true;
            }
        } else {
            self.ribbon_keyboard.pending = None;
        }
        if !allowed {
            self.ribbon_keyboard.pending = None;
        }
        if !intents.is_empty() {
            self.ribbon_keyboard.handled_frame = Some(frame_number);
        }
        if commands == Some(false)
            && intents
                .iter()
                .any(|intent| matches!(intent, Intent::Commands))
        {
            self.show_ribbon_temporarily();
        }
        let reveal = self.ribbon_keyboard.reveal.take().filter(|reveal| {
            reveal.scope == self.ribbon_keyboard_scope() && Some(reveal.owner) == focused
        });
        let frame = Frame {
            pass: ui.ctx().cumulative_pass_nr(),
            scope: self.ribbon_keyboard_scope(),
            tab: self.ribbon_tab,
            intents,
            origin: focused,
            retry,
            tabs: Vec::new(),
            groups: Vec::new(),
            active_group: None,
            reveal: reveal.map(|reveal| reveal.owner),
        };
        ui.ctx()
            .data_mut(|data| data.insert_temp(registry(), Some(frame)));
    }

    pub(in crate::workspace) fn finish_ribbon_keyboard(&mut self, ui: &Ui, actions: bool) {
        let Some(mut frame) = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<Option<Frame>>(registry()))
            .flatten()
        else {
            return;
        };
        if frame.pass != ui.ctx().cumulative_pass_nr()
            || frame.scope != self.ribbon_keyboard_scope()
            || !self.ribbon_keyboard_available(ui)
            || Popup::is_any_open(ui.ctx())
            || actions
        {
            frame.intents.clear();
        }
        if self.ribbon_collapsed() && !self.ribbon_temporarily_open() {
            frame.intents.retain(|intent| {
                !matches!(
                    intent,
                    Intent::Commands | Intent::Command(_) | Intent::Group(_)
                )
            });
        }
        let origin_current = frame
            .tabs
            .iter()
            .any(|tab| Some(tab.response.id) == frame.origin)
            || frame.groups.iter().any(|group| {
                group
                    .commands
                    .iter()
                    .any(|command| Some(command.id) == frame.origin)
            });
        let intents = mem::take(&mut frame.intents);
        for intent in intents {
            if !matches!(intent, Intent::Enter) && !origin_current {
                break;
            }
            self.navigate_ribbon(ui, &frame, intent);
        }
        frame.scope = self.ribbon_keyboard_scope();
        let focused = ui.ctx().memory(|memory| memory.focused());
        if focused != frame.origin
            && let Some(owner) = focused
        {
            let current = frame.tabs.iter().any(|tab| tab.response.id == owner)
                || frame
                    .groups
                    .iter()
                    .any(|group| group.commands.iter().any(|command| command.id == owner));
            if current {
                self.ribbon_keyboard.reveal = Some(Reveal {
                    scope: frame.scope,
                    owner,
                });
                ui.ctx().request_repaint();
            }
        }
        ui.ctx()
            .data_mut(|data| data.insert_temp(registry(), Some(frame)));
    }

    fn navigate_ribbon(&mut self, ui: &Ui, frame: &Frame, intent: Intent) {
        let focused = ui.ctx().memory(|memory| memory.focused());
        let tab_position = frame
            .tabs
            .iter()
            .position(|tab| Some(tab.response.id) == focused);
        let selected = frame
            .tabs
            .iter()
            .find(|tab| tab.tab == Some(self.ribbon_tab));
        match intent {
            Intent::Enter => {
                if let Some(tab) = selected {
                    focus(&tab.response);
                }
            }
            Intent::Tab(next) => {
                if let Some(position) = tab_position {
                    let target = &frame.tabs[neighbor(position, frame.tabs.len(), next)];
                    if let Some(tab) = target.tab {
                        self.ribbon_tab = tab;
                    }
                    focus(&target.response);
                }
            }
            Intent::Commands => {
                if tab_position
                    .is_some_and(|position| frame.tabs[position].tab == Some(self.ribbon_tab))
                {
                    let command = (frame.tab == self.ribbon_tab)
                        .then(|| frame.groups.iter().find_map(|group| group.commands.first()))
                        .flatten();
                    if let Some(command) = command {
                        focus(command);
                    } else if !frame.retry {
                        self.ribbon_keyboard.pending = Some(Entry {
                            scope: self.ribbon_keyboard_scope(),
                            focus_witness: focused,
                            display: (self.ribbon_collapsed(), self.ribbon_temporarily_open()),
                        });
                        ui.ctx().request_repaint();
                    }
                }
            }
            Intent::Command(next) => {
                let commands: Vec<_> = frame
                    .groups
                    .iter()
                    .flat_map(|group| &group.commands)
                    .collect();
                if let Some(position) = commands
                    .iter()
                    .position(|command| Some(command.id) == focused)
                {
                    focus(commands[neighbor(position, commands.len(), next)]);
                }
            }
            Intent::Group(next) => {
                let groups: Vec<_> = frame
                    .groups
                    .iter()
                    .filter(|group| !group.commands.is_empty())
                    .collect();
                if let Some(position) = groups.iter().position(|group| {
                    group
                        .commands
                        .iter()
                        .any(|command| Some(command.id) == focused)
                }) {
                    focus(&groups[neighbor(position, groups.len(), next)].commands[0]);
                }
            }
            Intent::Back => {
                if tab_position.is_some() {
                    self.return_to_document_from_ribbon(ui);
                } else if let Some(tab) = selected {
                    focus(&tab.response);
                }
            }
        }
    }

    pub(in crate::workspace) fn capture_ribbon_focus(&mut self, ui: &Ui) {
        let frame = ui
            .ctx()
            .data_mut(|data| data.remove_temp::<Option<Frame>>(registry()))
            .flatten();
        self.ribbon_keyboard.witness = None;
        let Some(frame) = frame else {
            return;
        };
        if frame.pass != ui.ctx().cumulative_pass_nr()
            || frame.scope != self.ribbon_keyboard_scope()
            || !self.ribbon_keyboard_available(ui)
            || Popup::is_any_open(ui.ctx())
        {
            return;
        }
        let focused = ui.ctx().memory(|memory| memory.focused());
        let commands_visible = !self.ribbon_collapsed() || self.ribbon_temporarily_open();
        let commands = commands_visible
            && frame.tab == self.ribbon_tab
            && frame.groups.iter().any(|group| {
                group
                    .commands
                    .iter()
                    .any(|command| Some(command.id) == focused)
            });
        let tab = frame
            .tabs
            .iter()
            .any(|tab| Some(tab.response.id) == focused);
        if (tab || commands)
            && let Some(owner) = focused
        {
            self.ribbon_keyboard.witness = Some(Witness {
                scope: frame.scope,
                owner,
                commands,
                frame: ui.ctx().cumulative_frame_nr(),
            });
        }
    }
}

fn neighbor(position: usize, length: usize, next: bool) -> usize {
    if next {
        (position + 1) % length
    } else {
        (position + length - 1) % length
    }
}

#[cfg(test)]
mod tests;
