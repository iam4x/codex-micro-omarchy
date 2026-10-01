use super::CodexMicro;
use crate::{
    daemon,
    model::{Action, Control, Phase, Preset, config_path},
};
use gpui::{App, Context, Entity, ParentElement, Styled, Window, div, point, px, rgb};
use gpui_component::{
    WindowExt, button::ButtonVariant, dialog::DialogButtonProps, input::InputState,
};

#[derive(Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ActionTab {
    System,
    App,
    Shortcut,
    Text,
    Command,
    Ai,
}
impl ActionTab {
    pub(super) const ALL: [Self; 6] = [
        Self::System,
        Self::App,
        Self::Shortcut,
        Self::Text,
        Self::Command,
        Self::Ai,
    ];
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::App => "App",
            Self::Shortcut => "Shortcut",
            Self::Text => "Text",
            Self::Command => "Command",
            Self::Ai => "AI",
        }
    }
    pub(super) fn icon(self) -> &'static str {
        match self {
            Self::System => "settings",
            Self::App => "globe",
            Self::Shortcut => "keyboard",
            Self::Text => "text",
            Self::Command => "code",
            Self::Ai => "sparkles",
        }
    }
}

impl CodexMicro {
    pub(super) fn select(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        if self.lighting.page {
            let target = match control {
                Control::AG00 => 0,
                Control::AG01 => 1,
                Control::AG02 => 2,
                Control::AG03 => 3,
                Control::AG04 => 4,
                Control::AG05 => 5,
                Control::ACT06
                | Control::ACT07
                | Control::ACT08
                | Control::ACT09
                | Control::Mic
                | Control::ACT12 => 6,
                _ => return,
            };
            self.select_light(target, window, cx);
            return;
        }
        self.shortcut_capture.reset();
        self.selected = control;
        self.phase = control.primary_phase();
        self.load_editor(window, cx);
        self.message = format!("Editing {}", control.name());
        self.message_error = false;
        cx.notify();
    }
    pub(super) fn load_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.invalidate_ai();
        self.shortcut_capture.reset();
        self.text_submit = false;
        let action = self
            .profiles
            .active()
            .action(self.selected, self.phase)
            .cloned();
        let text = match action {
            Some(Action::Preset { preset }) => {
                self.tab = ActionTab::System;
                self.preset = preset;
                String::new()
            }
            Some(Action::Launch { command }) => {
                self.tab = ActionTab::App;
                command
            }
            Some(Action::Shortcut { chord }) => {
                self.tab = ActionTab::Shortcut;
                chord
            }
            Some(Action::Text { text, submit }) => {
                self.tab = ActionTab::Text;
                self.text_submit = submit;
                text
            }
            Some(Action::Command { command }) => {
                self.tab = ActionTab::Command;
                command
            }
            Some(Action::Ai {
                prompt,
                summary,
                script,
            }) => {
                self.tab = ActionTab::Ai;
                self.ai_state = super::ai::AiState::Ready {
                    prompt: prompt.clone(),
                    generated: crate::ai::GeneratedScript { summary, script },
                };
                prompt
            }
            None => {
                self.tab = ActionTab::System;
                self.preset = Preset::Terminal;
                String::new()
            }
        };
        self.action_input()
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }
    pub(super) fn switch_tab(
        &mut self,
        tab: ActionTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.invalidate_ai();
        self.tab = tab;
        self.text_submit = false;
        self.action_scroll.set_offset(point(px(0.), px(0.)));
        self.shortcut_capture.reset();
        self.action_input().update(cx, |input, cx| {
            input.set_value("", window, cx);
            if !matches!(tab, ActionTab::System | ActionTab::Shortcut)
                || (tab == ActionTab::Shortcut && self.shortcut_manual)
            {
                input.focus(window, cx);
            }
        });
        if tab == ActionTab::Shortcut && !self.shortcut_manual {
            window.focus(&self.shortcut_focus);
        }
        cx.notify();
    }
    pub(super) fn switch_phase(&mut self, phase: Phase, cx: &mut Context<Self>) {
        if self.phase == phase {
            return;
        }
        self.phase = phase;
        cx.notify();
    }
    pub(super) fn set_text_submit(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.text_submit = enabled;
        cx.notify();
    }
    pub(super) fn reset_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profiles.active().bindings.is_empty() {
            return;
        }
        let mut updated = self.profiles.clone();
        updated.active_mut().bindings.clear();
        match updated.save(&config_path()) {
            Ok(()) => {
                self.reset_backup = Some(self.profiles.active().clone());
                self.profiles = updated;
                self.load_editor(window, cx);
                self.message = "All bindings removed. Use Undo reset to restore them.".into();
                self.message_error = false;
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn undo_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(previous) = self.reset_backup.as_ref() else {
            return;
        };
        let mut updated = self.profiles.clone();
        updated.active_mut().bindings = previous.bindings.clone();
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.reset_backup = None;
                self.load_editor(window, cx);
                self.message = "Bindings restored".into();
                self.message_error = false;
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn action_input(&self) -> &Entity<InputState> {
        match self.tab {
            ActionTab::Text => &self.text_input,
            ActionTab::Ai => &self.ai_prompt,
            _ => &self.input,
        }
    }
    pub(super) fn is_dirty(&self, cx: &App) -> bool {
        self.profiles.active().action(self.selected, self.phase) != Some(&self.draft(cx))
    }
    pub(super) fn draft(&self, cx: &App) -> Action {
        let text = self.action_input().read(cx).value().to_string();
        match self.tab {
            ActionTab::System => Action::Preset {
                preset: self.preset,
            },
            ActionTab::App => Action::Launch { command: text },
            ActionTab::Shortcut => Action::Shortcut { chord: text },
            ActionTab::Text => Action::Text {
                text,
                submit: self.text_submit,
            },
            ActionTab::Command => Action::Command { command: text },
            ActionTab::Ai => Action::Ai {
                prompt: text,
                summary: self
                    .ai_state
                    .output()
                    .map(|output| output.summary.clone())
                    .unwrap_or_default(),
                script: self
                    .ai_state
                    .output()
                    .map(|output| output.script.clone())
                    .unwrap_or_default(),
            },
        }
    }
    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_dialog {
            return;
        }
        if self.lighting.page {
            self.save_lighting(cx);
            return;
        }
        if self.tab == ActionTab::Ai && !self.ai_can_run(cx) {
            self.message = "Generate a script first, then save the binding.".into();
            self.message_error = true;
            cx.notify();
            return;
        }
        if self.tab == ActionTab::Shortcut && self.shortcut_capture.is_held() {
            self.message = "Release all keys to finish recording first.".into();
            self.message_error = true;
            cx.notify();
            return;
        }
        let action = self.draft(cx);
        if let Err(error) = action.argv() {
            self.message = format!("{error:#}");
            self.message_error = true;
            cx.notify();
            return;
        }
        let mut updated = self.profiles.clone();
        updated
            .active_mut()
            .assign(self.selected, self.phase, Some(action.clone()));
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.reset_backup = None;
                self.message = format!("{} assigned to {}", action.title(), self.selected.name());
                self.message_error = false;
                window.blur();
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_editor(window, cx);
        self.message = "Changes discarded".into();
        self.message_error = false;
        cx.notify();
    }
    pub(super) fn confirm_remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.profiles.active().action(self.selected, self.phase) else {
            return;
        };
        let view = cx.entity();
        let muted = rgb(self.palette.muted);
        let title = action.title();
        let target = if self.selected.is_rotation() {
            self.selected.name().to_string()
        } else {
            format!("{} · {}", self.selected.name(), self.phase.label())
        };
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .confirm()
                .w(px(380.))
                .title("Remove binding?")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(format!("{target}  ·  {title}"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("The button will do nothing until you assign a new action."),
                        ),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Remove")
                        .ok_variant(ButtonVariant::Danger),
                )
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| this.remove(window, cx));
                    true
                })
        });
    }
    pub(super) fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut updated = self.profiles.clone();
        updated.active_mut().assign(self.selected, self.phase, None);
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.reset_backup = None;
                self.load_editor(window, cx);
                self.message = "Binding removed".into();
                self.message_error = false;
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn test(&mut self, cx: &mut Context<Self>) {
        if self.tab == ActionTab::Ai && !self.ai_can_run(cx) {
            self.message = "Generate a script first, then test the action.".into();
            self.message_error = true;
            cx.notify();
            return;
        }
        let action = self.draft(cx);
        if let Err(error) = action.argv() {
            self.message = format!("{error:#}");
            self.message_error = true;
            cx.notify();
            return;
        }
        self.message = "Testing action…".into();
        self.message_error = false;
        cx.spawn(async move |view, cx| {
            let result = smol::unblock(move || daemon::test(&action)).await;
            let _ = view.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        this.message = "Action started. Check the activity below.".into();
                        this.message_error = false;
                    }
                    Err(error) => {
                        this.message = format!("{error:#}");
                        this.message_error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
