//! Window keyboard shortcuts that are not tied to one widget, and the list
//! opened with F1 or `?`. Global (system-wide) hotkeys are not registered:
//! Wayland has no portable way to do it.

use super::*;
use iced::widget::{column, container, row, text};
use iced::{keyboard, Alignment, Length};

/// Keys and what they do, in the order shown in the list. `Ctrl` is shown as
/// `⌘` on macOS.
const SHORTCUTS: [(&str, &str); 13] = [
    ("← / →", "Previous / next day"),
    ("T", "Today"),
    ("+ / −", "Zoom in / out"),
    ("Enter", "Tag the selection"),
    ("Esc", "Clear the selection"),
    ("Alt+↑ / ↓", "Previous / next timeline block"),
    ("Ctrl+F", "Search"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+1…5", "Day, Week, Tags, Rules, Settings"),
    ("P", "Pause for the last chosen time / resume"),
    ("N", "Tag last N minutes…"),
    ("F6", "Move focus to the navigation rail"),
    ("F1 / ?", "Keyboard shortcuts"),
];

fn key_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        key.replace("Ctrl+", "⌘")
    } else {
        key.into()
    }
}

/// A shortcut handled by [`State::shortcut`].
#[derive(Debug, PartialEq)]
pub enum Action {
    Help,
    Screen(&'static str),
    TogglePause,
    QuickTag,
}

/// Map a key press that no widget used to an app-wide action.
pub fn action(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Action> {
    use keyboard::key::Named;
    if *key == keyboard::Key::Named(Named::F1) {
        return Some(Action::Help);
    }
    let keyboard::Key::Character(c) = key else {
        return None;
    };
    if modifiers.command() && !modifiers.alt() {
        let index = c.parse::<usize>().ok().filter(|i| (1..=5).contains(i))?;
        return Some(Action::Screen(
            ["Day", "Week", "Tags", "Rules", "Settings"][index - 1],
        ));
    }
    if modifiers.command() || modifiers.alt() || modifiers.control() {
        return None;
    }
    match c.as_str() {
        "?" => Some(Action::Help),
        "p" | "P" => Some(Action::TogglePause),
        "n" | "N" => Some(Action::QuickTag),
        _ => None,
    }
}

impl State {
    pub(super) fn shortcut(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::Help => {
                self.shortcuts_open = true;
                Task::none()
            }
            Action::Screen(screen) => self.update(Message::Screen(screen)),
            Action::TogglePause => self.update(if self.status.state == "paused" {
                Message::Resume
            } else {
                Message::Pause(Some(tray::last_pause() * 60_000))
            }),
            Action::QuickTag => self.update(Message::Edit(tagging::Edit::QuickCustom)),
        }
    }

    pub(super) fn shortcuts_dialog(&self) -> Element<'_, Message> {
        let p = design::Palette::new(self.theme.palette().background.r < 0.5);
        let mut list = column![].spacing(design::SMALL_GAP);
        for (key, description) in SHORTCUTS {
            list = list.push(
                row![
                    container(text(key_label(key)).font(design::MONO).size(design::BODY))
                        .width(Length::Fixed(design::SHORTCUT_KEY)),
                    text(tr(description)).size(design::BODY).color(p.text2),
                ]
                .spacing(design::GAP)
                .align_y(Alignment::Center),
            );
        }
        container(
            column![
                text(tr("Keyboard shortcuts")).size(design::HEADING),
                list,
                text(tr("Shortcuts work when no text field is focused."))
                    .size(design::LABEL)
                    .color(p.text3),
                button(text(tr("Close")))
                    .style(design::primary)
                    .on_press(Message::CloseShortcuts),
            ]
            .spacing(design::GAP),
        )
        .padding(design::PADDING)
        .max_width(design::SETTINGS_DIALOG_WIDTH)
        .style(panel)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyboard::{key::Named, Key, Modifiers};

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn keys_map_to_actions_only_without_conflicting_modifiers() {
        assert_eq!(
            action(&Key::Named(Named::F1), Modifiers::empty()),
            Some(Action::Help)
        );
        assert_eq!(action(&ch("?"), Modifiers::SHIFT), Some(Action::Help));
        assert_eq!(
            action(&ch("p"), Modifiers::empty()),
            Some(Action::TogglePause)
        );
        assert_eq!(action(&ch("N"), Modifiers::SHIFT), Some(Action::QuickTag));
        assert_eq!(action(&ch("p"), Modifiers::COMMAND), None);
        assert_eq!(action(&ch("n"), Modifiers::ALT), None);
        for (index, screen) in ["Day", "Week", "Tags", "Rules", "Settings"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                action(&ch(&(index + 1).to_string()), Modifiers::COMMAND),
                Some(Action::Screen(screen))
            );
        }
        assert_eq!(action(&ch("6"), Modifiers::COMMAND), None);
        assert_eq!(action(&ch("1"), Modifiers::empty()), None);
        assert_eq!(action(&ch("x"), Modifiers::empty()), None);
    }

    fn press(state: &mut State, key: Key, modifiers: Modifiers) {
        let _ = state.update(Message::Key(key, modifiers));
    }

    #[test]
    fn shortcuts_switch_screens_open_help_and_quick_tag() {
        let (mut state, _) = State::boot();
        state.fixture = true;
        press(&mut state, ch("2"), Modifiers::COMMAND);
        assert_eq!(state.screen, "Week");
        press(&mut state, ch("5"), Modifiers::COMMAND);
        assert_eq!(state.screen, "Settings");
        press(&mut state, ch("1"), Modifiers::COMMAND);
        assert_eq!(state.screen, "Day");

        press(&mut state, Key::Named(Named::F1), Modifiers::empty());
        assert!(state.shortcuts_open);
        // Other shortcuts are inert while the list is open.
        press(&mut state, ch("2"), Modifiers::COMMAND);
        assert_eq!(state.screen, "Day");
        press(&mut state, Key::Named(Named::Escape), Modifiers::empty());
        assert!(!state.shortcuts_open);
        press(&mut state, ch("?"), Modifiers::SHIFT);
        assert!(state.shortcuts_open);
        let _ = state.update(Message::CloseShortcuts);
        assert!(!state.shortcuts_open);

        press(&mut state, ch("n"), Modifiers::empty());
        let editor = state.editor.as_ref().expect("quick tag editor");
        assert!(editor.minutes.is_empty());
        // Letters go to the open editor, not to shortcuts.
        press(&mut state, ch("?"), Modifiers::SHIFT);
        assert!(!state.shortcuts_open);
    }

    #[test]
    fn every_shortcut_description_is_translated() {
        // The catalog test checks every language once a key is in English.
        for (_, description) in SHORTCUTS {
            assert!(
                localization::catalogs()["en"][description].is_string(),
                "{description} missing"
            );
        }
    }
}
