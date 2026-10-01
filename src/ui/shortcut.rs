use super::{ActionTab, CodexMicro};
use gpui::{Context, KeyDownEvent, KeyUpEvent, ModifiersChangedEvent, Window};

use gpui::{Keystroke, Modifiers};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct ShortcutCapture {
    held_keys: BTreeSet<String>,
    modifiers: Modifiers,
    physical_modifiers: bool,
    candidate: Option<Keystroke>,
    multiple_keys: bool,
}

impl ShortcutCapture {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn key_down(&mut self, stroke: &Keystroke) {
        if !self.physical_modifiers {
            self.modifiers = stroke.modifiers;
        }
        let key = stroke.key.to_lowercase();
        if is_modifier(&key) {
            return;
        }
        self.held_keys.insert(physical_key(&key).into());
        if let Some(candidate) = &mut self.candidate {
            self.multiple_keys |= physical_key(&candidate.key) != physical_key(&key);
        } else {
            self.candidate = Some(Keystroke {
                key,
                ..stroke.clone()
            });
        }
        self.include_modifiers();
    }

    pub fn key_up(&mut self, stroke: &Keystroke) -> Option<Result<String, &'static str>> {
        self.held_keys
            .remove(physical_key(&stroke.key.to_lowercase()));
        if !self.physical_modifiers {
            self.modifiers = stroke.modifiers;
        }
        self.finish()
    }

    pub fn modifiers_changed(
        &mut self,
        modifiers: Modifiers,
    ) -> Option<Result<String, &'static str>> {
        self.modifiers = modifiers;
        self.physical_modifiers = true;
        self.include_modifiers();
        self.finish()
    }

    fn include_modifiers(&mut self) {
        if let Some(candidate) = &mut self.candidate {
            candidate.modifiers.control |= self.modifiers.control;
            candidate.modifiers.alt |= self.modifiers.alt;
            candidate.modifiers.shift |= self.modifiers.shift;
            candidate.modifiers.platform |= self.modifiers.platform;
        }
    }

    pub fn is_held(&self) -> bool {
        !self.held_keys.is_empty() || self.modifiers.modified()
    }

    pub fn preview(&self) -> Option<String> {
        if self.multiple_keys {
            Some("Use modifiers with one key".into())
        } else if let Some(candidate) = &self.candidate {
            Some(chord(&candidate.modifiers, Some(&candidate.key)))
        } else if self.modifiers.modified() {
            Some(chord(&self.modifiers, None))
        } else {
            None
        }
    }

    fn finish(&mut self) -> Option<Result<String, &'static str>> {
        if self.is_held() {
            return None;
        }
        let candidate = self.candidate.take()?;
        let result = if self.multiple_keys {
            Err("Use Ctrl, Alt, Shift, or Super with a single key.")
        } else {
            Ok(chord(&candidate.modifiers, Some(&candidate.key)))
        };
        self.reset();
        Some(result)
    }
}

fn is_modifier(key: &str) -> bool {
    matches!(
        key,
        "control" | "ctrl" | "shift" | "alt" | "super" | "meta" | "fn"
    )
}

// GPUI reports symbols, so Shift can change a held key's name before release.
fn physical_key(key: &str) -> &str {
    match key {
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        "_" => "-",
        "+" => "=",
        "{" => "[",
        "}" => "]",
        "|" => "\\",
        ":" => ";",
        "\"" => "'",
        "<" => ",",
        ">" => ".",
        "?" => "/",
        "~" => "`",
        key => key,
    }
}

fn chord(modifiers: &Modifiers, key: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (held, name) in [
        (modifiers.control, "Ctrl"),
        (modifiers.alt, "Alt"),
        (modifiers.shift, "Shift"),
        (modifiers.platform, "Super"),
    ] {
        if held {
            parts.push(name.into());
        }
    }
    if let Some(key) = key {
        parts.push(match key {
            "enter" => "Return".into(),
            "escape" => "Escape".into(),
            "space" => "space".into(),
            "backspace" => "BackSpace".into(),
            "pageup" => "Prior".into(),
            "pagedown" => "Next".into(),
            "print" | "printscreen" => "Print".into(),
            "caps_lock" | "capslock" => "Caps_Lock".into(),
            "!" => "exclam".into(),
            "\"" => "quotedbl".into(),
            "#" => "numbersign".into(),
            "$" => "dollar".into(),
            "%" => "percent".into(),
            "&" => "ampersand".into(),
            "'" => "apostrophe".into(),
            "(" => "parenleft".into(),
            ")" => "parenright".into(),
            "*" => "asterisk".into(),
            "+" => "plus".into(),
            "," => "comma".into(),
            "-" => "minus".into(),
            "." => "period".into(),
            "/" => "slash".into(),
            ":" => "colon".into(),
            ";" => "semicolon".into(),
            "<" => "less".into(),
            "=" => "equal".into(),
            ">" => "greater".into(),
            "?" => "question".into(),
            "@" => "at".into(),
            "[" => "bracketleft".into(),
            "\\" => "backslash".into(),
            "]" => "bracketright".into(),
            "^" => "asciicircum".into(),
            "_" => "underscore".into(),
            "`" => "grave".into(),
            "{" => "braceleft".into(),
            "|" => "bar".into(),
            "}" => "braceright".into(),
            "~" => "asciitilde".into(),
            key if key.len() == 1
                || key
                    .strip_prefix('f')
                    .is_some_and(|n| n.parse::<u8>().is_ok()) =>
            {
                key.to_uppercase()
            }
            key => {
                let mut chars = key.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            }
        });
    }
    parts.join("+")
}

impl CodexMicro {
    pub(super) fn shortcut_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab != ActionTab::Shortcut || !self.shortcut_focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        self.shortcut_capture.key_down(&event.keystroke);
        cx.notify();
    }
    pub(super) fn clear_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shortcut_capture.reset();
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        window.focus(&self.shortcut_focus);
        self.message = "Shortcut cleared. Press a new shortcut.".into();
        self.message_error = false;
        cx.notify();
    }
    pub(super) fn shortcut_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab != ActionTab::Shortcut || !self.shortcut_focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        let captured = self.shortcut_capture.key_up(&event.keystroke);
        self.finish_shortcut(captured, window, cx);
    }
    pub(super) fn shortcut_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab != ActionTab::Shortcut || !self.shortcut_focus.is_focused(window) {
            return;
        }
        let captured = self.shortcut_capture.modifiers_changed(event.modifiers);
        self.finish_shortcut(captured, window, cx);
    }
    fn finish_shortcut(
        &mut self,
        captured: Option<Result<String, &'static str>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(result) = captured {
            match result {
                Ok(chord) => {
                    self.input
                        .update(cx, |input, cx| input.set_value(chord, window, cx));
                    self.message = "Shortcut captured. Save binding to assign it.".into();
                    self.message_error = false;
                }
                Err(error) => {
                    self.message = error.into();
                    self.message_error = true;
                }
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stroke(key: &str) -> Keystroke {
        Keystroke::parse(key).unwrap()
    }

    #[test]
    fn unrelated_releases_cannot_finish_a_held_key() {
        let mut capture = ShortcutCapture::default();
        capture.modifiers_changed(stroke("ctrl-c").modifiers);
        capture.key_down(&stroke("ctrl-c"));
        assert!(capture.modifiers_changed(Modifiers::default()).is_none());
        assert!(capture.key_up(&stroke("shift")).is_none());
        assert!(capture.key_up(&stroke("v")).is_none());
        assert!(capture.is_held());
        assert_eq!(capture.key_up(&stroke("c")), Some(Ok("Ctrl+C".into())));
    }

    #[test]
    fn shifted_repeat_is_one_physical_key_and_unrelated_release_is_ignored() {
        let mut capture = ShortcutCapture::default();
        capture.modifiers_changed(stroke("shift-c").modifiers);
        capture.key_down(&stroke("?"));
        capture.modifiers_changed(Modifiers::default());
        capture.key_down(&stroke("/"));
        assert!(capture.key_up(&stroke("v")).is_none());
        assert_eq!(
            capture.key_up(&stroke("/")),
            Some(Ok("Shift+question".into()))
        );
    }

    #[test]
    fn waits_for_modifier_release_after_normal_key() {
        let mut capture = ShortcutCapture::default();
        capture.key_down(&stroke("ctrl-shift-c"));
        assert_eq!(capture.preview().as_deref(), Some("Ctrl+Shift+C"));
        assert!(capture.key_up(&stroke("ctrl-shift-c")).is_none());
        assert!(
            capture
                .modifiers_changed(stroke("ctrl-c").modifiers)
                .is_none()
        );
        assert_eq!(
            capture.modifiers_changed(Modifiers::default()),
            Some(Ok("Ctrl+Shift+C".into()))
        );
        assert!(capture.preview().is_none());
    }

    #[test]
    fn modifier_first_release_preserves_the_recorded_combination() {
        let mut capture = ShortcutCapture::default();
        capture.key_down(&stroke("alt-enter"));
        assert!(capture.modifiers_changed(Modifiers::default()).is_none());
        assert_eq!(
            capture.key_up(&stroke("enter")),
            Some(Ok("Alt+Return".into()))
        );
    }

    #[test]
    fn handles_repeat_modifiers_added_after_key_and_modifier_only_input() {
        let mut capture = ShortcutCapture::default();
        capture.key_down(&stroke("c"));
        capture.key_down(&stroke("c"));
        capture.modifiers_changed(stroke("ctrl-c").modifiers);
        assert!(capture.key_up(&stroke("ctrl-c")).is_none());
        assert_eq!(
            capture.modifiers_changed(Modifiers::default()),
            Some(Ok("Ctrl+C".into()))
        );
        assert!(
            capture
                .modifiers_changed(stroke("shift-c").modifiers)
                .is_none()
        );
        assert!(capture.modifiers_changed(Modifiers::default()).is_none());
    }

    #[test]
    fn shifted_symbol_waits_for_physical_shift_and_serializes_without_delimiter_ambiguity() {
        let mut capture = ShortcutCapture::default();
        capture.modifiers_changed(stroke("shift-c").modifiers);
        // GPUI removes consumed Shift from symbol keystrokes on Wayland.
        capture.key_down(&stroke("+"));
        assert!(capture.key_up(&stroke("+")).is_none());
        let value = capture
            .modifiers_changed(Modifiers::default())
            .unwrap()
            .unwrap();
        assert_eq!(value, "Shift+plus");
        assert!(crate::model::parse_shortcut(&value).is_ok());
        capture.modifiers_changed(stroke("shift-c").modifiers);
        capture.key_down(&stroke("?"));
        assert!(capture.modifiers_changed(Modifiers::default()).is_none());
        assert_eq!(
            capture.key_up(&stroke("/")),
            Some(Ok("Shift+question".into()))
        );
    }

    #[test]
    fn rejects_multiple_keys_and_reset_cancels_pending_capture() {
        let mut capture = ShortcutCapture::default();
        capture.key_down(&stroke("c"));
        capture.key_down(&stroke("v"));
        assert!(capture.key_up(&stroke("c")).is_none());
        assert!(capture.key_up(&stroke("v")).unwrap().is_err());
        capture.key_down(&stroke("ctrl-c"));
        capture.reset();
        assert!(capture.key_up(&stroke("c")).is_none());
    }
}
