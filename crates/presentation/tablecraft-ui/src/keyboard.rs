//! Shared native/web workspace shortcuts, with actions admitted by view state.
use egui::{Context, Key, Modifiers, os::OperatingSystem};

#[derive(Default)]
pub(crate) struct Shortcuts {
    pub focus_path: bool,
    pub find: bool,
    pub query: bool,
    pub cancel: bool,
}

pub(crate) fn consume(ctx: &Context, query_ready: bool, busy: bool, find_ready: bool) -> Shortcuts {
    ctx.input_mut(|input| {
        let find = find_ready && input.consume_key(Modifiers::COMMAND, Key::F);
        let focus_path = input.consume_key(Modifiers::COMMAND, Key::O);
        let query = query_ready && input.consume_key(Modifiers::COMMAND, Key::Enter);
        let cancel = busy && input.consume_key(Modifiers::NONE, Key::Escape);
        Shortcuts {
            focus_path,
            find,
            query,
            cancel,
        }
    })
}

/// Access object-window shortcuts, distinct from plain F6 pane navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ObjectShortcut {
    Next,
    Previous,
    Close,
}

/// Remove recognized object chords before text widgets can reinterpret them.
/// In particular egui treats Ctrl+W as word deletion on every OS, whereas Access
/// uses it to close an object on Windows. macOS keeps Control+W text editing and
/// uses Command for the explicit TableCraft object-command adaptation.
pub(crate) fn consume_object_shortcut(ctx: &Context) -> Option<ObjectShortcut> {
    let os = ctx.os();
    let (shortcut, close_requested) = ctx.input_mut(|input| {
        let mut first = None;
        let mut close_requested = false;
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                repeat,
                ..
            } = event
            else {
                return true;
            };
            let Some(shortcut) = object_shortcut(os, *key, *modifiers) else {
                return true;
            };
            close_requested |= shortcut == ObjectShortcut::Close;
            if !repeat && first.is_none() {
                first = Some(shortcut);
            }
            false
        });
        (first, close_requested)
    });
    if close_requested {
        // This chord owns an object, even when object close is refused. It must
        // never fall through to an integration's native app-window close.
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    }
    shortcut
}

fn object_shortcut(os: OperatingSystem, key: Key, modifiers: Modifiers) -> Option<ObjectShortcut> {
    let mac = os == OperatingSystem::Mac;
    let command = if mac {
        modifiers.mac_cmd && !modifiers.ctrl
    } else {
        modifiers.ctrl && !modifiers.mac_cmd
    };
    // `command` is egui's semantic mirror; physical Ctrl/Cmd, Shift and Alt
    // determine this exact chord, including raw integrations without the mirror.
    if !command || modifiers.alt {
        return None;
    }
    match (key, modifiers.shift) {
        (Key::F6, false) => Some(ObjectShortcut::Next),
        (Key::F6, true) => Some(ObjectShortcut::Previous),
        (Key::W, false) => Some(ObjectShortcut::Close),
        (Key::F4, false) if matches!(os, OperatingSystem::Windows | OperatingSystem::Nix) => {
            Some(ObjectShortcut::Close)
        }
        _ => None,
    }
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
                result = Some(consume(ui.ctx(), query_ready, busy, false));
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
