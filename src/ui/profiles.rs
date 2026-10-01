use super::CodexMicro;
use crate::model::config_path;
use gpui::{Context, Window, div, prelude::*, px, rgb};
use gpui_component::{WindowExt, dialog::DialogButtonProps, input::Input};

impl CodexMicro {
    pub(super) fn open_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_dialog {
            return;
        }
        self.shortcut_capture.reset();
        self.profile_error.update(cx, |error, _| *error = None);
        self.profile_name
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.profile_dialog = true;
        let view = cx.weak_entity();
        let input = self.profile_name.clone();
        let p = self.palette;
        let error_state = self.profile_error.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let error = error_state.read(cx).clone();
            let confirm_view = view.clone();
            let close_view = view.clone();
            dialog
                .title("Create a profile")
                .confirm()
                .w(px(400.))
                .bg(rgb(p.background))
                .text_color(rgb(p.text))
                .button_props(DialogButtonProps::default().ok_text("Create profile"))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_size(px(12.)).child("Profile name"))
                        .child(Input::new(&input).w_full())
                        .when_some(error, |form, error| {
                            form.child(div().text_size(px(12.)).text_color(rgb(p.red)).child(error))
                        }),
                )
                .on_ok(move |_, _, cx| {
                    confirm_view
                        .update(cx, |this, cx| this.create_profile(cx))
                        .unwrap_or(false)
                })
                .on_close(move |_, _, cx| {
                    let _ = close_view.update(cx, |this, cx| {
                        this.profile_dialog = false;
                        this.profile_error.update(cx, |error, _| *error = None);
                        cx.notify();
                    });
                })
        });
        self.profile_name
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(super) fn cancel_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.profile_dialog {
            return;
        }
        self.profile_dialog = false;
        self.profile_error.update(cx, |error, _| *error = None);
        window.close_dialog(cx);
        cx.notify();
    }

    pub(super) fn submit_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.create_profile(cx) {
            self.profile_dialog = false;
            window.close_dialog(cx);
            cx.notify();
        }
    }

    fn create_profile(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.profile_dialog {
            return false;
        }
        let name = self.profile_name.read(cx).value().to_string();
        let mut updated = self.profiles.clone();
        let result = updated
            .add(&name)
            .and_then(|index| updated.save(&config_path()).map(|_| index));
        match result {
            Ok(index) => {
                self.message = format!("Profile {} created", updated.list()[index].name);
                self.message_error = false;
                self.profiles = updated;
                self.profile_error.update(cx, |error, _| *error = None);
                cx.notify();
                true
            }
            Err(error) => {
                self.profile_error
                    .update(cx, |message, _| *message = Some(format!("{error:#}")));
                cx.notify();
                false
            }
        }
    }

    pub(super) fn activate_profile(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!self.profile_dialog, "Close the profile dialog first");
        if self.profiles.active_index() == index {
            return Ok(());
        }
        let mut updated = self.profiles.clone();
        updated.activate(index)?;
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.reset_backup = None;
                self.load_editor(window, cx);
                self.load_lighting(window, cx);
                self.message = format!("Profile {} active", self.profiles.active().name);
                self.message_error = false;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
                cx.notify();
                Err(error)
            }
        }
    }
}
