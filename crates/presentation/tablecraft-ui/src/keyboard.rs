//! Shared native/web workspace shortcuts, with actions admitted by view state.
use egui::{Context, Key, Modifiers};

#[derive(Default)]
pub(crate) struct Shortcuts {
    pub focus_path: bool,
    pub query: bool,
    pub cancel: bool,
}

pub(crate) fn consume(ctx: &Context, query_ready: bool, busy: bool) -> Shortcuts {
    ctx.input_mut(|input| {
        let focus_path = input.consume_key(Modifiers::COMMAND, Key::O);
        let query = query_ready && input.consume_key(Modifiers::COMMAND, Key::Enter);
        let cancel = busy && input.consume_key(Modifiers::NONE, Key::Escape);
        Shortcuts {
            focus_path,
            query,
            cancel,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, command: bool, query_ready: bool, busy: bool) -> Shortcuts {
        let ctx = Context::default();
        let modifiers = if command {
            Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            }
        } else {
            Modifiers::NONE
        };
        let mut result = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| {
                result = Some(consume(ui.ctx(), query_ready, busy));
            },
        );
        output.textures_delta.clear();
        result.unwrap()
    }

    #[test]
    fn command_open_focuses_path_without_submitting_open_or_query() {
        let shortcuts = press(Key::O, true, true, false);
        assert!(shortcuts.focus_path);
        assert!(!shortcuts.query);
        assert!(!shortcuts.cancel);
        assert!(!press(Key::O, false, true, false).focus_path);
    }

    #[test]
    fn query_and_cancel_shortcuts_obey_their_actual_admission_state() {
        assert!(press(Key::Enter, true, true, false).query);
        assert!(!press(Key::Enter, true, false, false).query);
        assert!(!press(Key::Enter, false, true, false).query);
        assert!(press(Key::Escape, false, false, true).cancel);
        assert!(!press(Key::Escape, false, false, false).cancel);
    }
}
