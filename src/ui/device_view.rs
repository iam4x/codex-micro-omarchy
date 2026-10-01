use super::CodexMicro;
use crate::{
    daemon::DeviceStatus,
    model::{Action, Control},
};
use gpui::{Context, Div, FontWeight, SharedString, Stateful, div, prelude::*, px, rgb};

impl CodexMicro {
    fn key(&self, control: Control, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette;
        let selected = self.selected == control;
        let active = self.active.is_some_and(|(key, _)| key == control);
        let action = self
            .profiles
            .active()
            .action(control, control.primary_phase());
        let title = action
            .map(Action::title)
            .unwrap_or_else(|| "Unassigned".into());
        let title: String = title
            .chars()
            .take(if width > 100. { 24 } else { 13 })
            .collect();
        div()
            .id(control.id())
            .w(px(width))
            .h(px(86.))
            .flex_shrink_0()
            .border_1()
            .rounded(px(4.))
            .border_color(rgb(if selected || active {
                p.accent
            } else {
                p.border
            }))
            .bg(rgb(if active {
                0x2c3945
            } else if selected {
                0x202a33
            } else {
                p.raised
            }))
            .cursor_pointer()
            .hover(|style| style.border_color(rgb(p.accent)))
            .px_3()
            .py_3()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_family("CaskaydiaMono Nerd Font")
                            .text_color(rgb(if selected { p.accent } else { p.text }))
                            .child(control.short_name()),
                    )
                    .child(
                        div()
                            .size(px(4.))
                            .rounded_full()
                            .bg(rgb(if action.is_some() { p.accent } else { p.border })),
                    ),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(if action.is_some() { p.text } else { p.muted }))
                    .child(title),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.select(control, window, cx)))
    }

    fn joystick_direction(
        &self,
        control: Control,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let p = self.palette;
        let selected = self.selected == control;
        let active = self.active.is_some_and(|(key, _)| key == control);
        let assigned = self.profiles.active().bindings.contains_key(&control);
        div()
            .id(control.id())
            .w(px(30.))
            .h(px(24.))
            .border_1()
            .rounded(px(3.))
            .border_color(rgb(if selected || active {
                p.accent
            } else {
                p.border
            }))
            .bg(rgb(if active {
                0x2c3945
            } else if selected {
                0x202a33
            } else {
                p.panel
            }))
            .text_color(rgb(if selected || assigned {
                p.accent
            } else {
                p.muted
            }))
            .text_size(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.border_color(rgb(p.accent)))
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| this.select(control, window, cx)))
    }

    fn board(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        div()
            .w(px(468.))
            .p_5()
            .flex_shrink_0()
            .bg(rgb(0x191919))
            .border_1()
            .border_color(rgb(p.border))
            .rounded(px(9.))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(px(98.))
                            .h(px(86.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .id("dial-top")
                                    .size(px(78.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(if self.selected == Control::DialPress {
                                        p.accent
                                    } else {
                                        p.border
                                    }))
                                    .bg(rgb(p.panel))
                                    .cursor_pointer()
                                    .hover(|style| style.border_color(rgb(p.accent)))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .size(px(50.))
                                            .rounded_full()
                                            .border_1()
                                            .border_color(rgb(p.border))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap_1()
                                            .child(div().w(px(2.)).h(px(8.)).bg(rgb(p.accent)))
                                            .child(self.icon("dial", 18., p.muted)),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select(Control::DialPress, window, cx)
                                    })),
                            ),
                    )
                    .child(self.key(Control::AG00, 98., cx))
                    .child(self.key(Control::AG01, 98., cx))
                    .child(
                        div()
                            .w(px(98.))
                            .h(px(86.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_1()
                            .child(self.joystick_direction(Control::JoystickUp, "↑", cx))
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(self.joystick_direction(Control::JoystickLeft, "←", cx))
                                    .child(div().size(px(18.)).rounded_full().bg(rgb(p.raised)))
                                    .child(self.joystick_direction(
                                        Control::JoystickRight,
                                        "→",
                                        cx,
                                    )),
                            )
                            .child(self.joystick_direction(Control::JoystickDown, "↓", cx)),
                    ),
            )
            .child(
                div().flex().gap_3().children(
                    [Control::AG02, Control::AG03, Control::AG04, Control::AG05]
                        .into_iter()
                        .map(|control| self.key(control, 98., cx)),
                ),
            )
            .child(
                div().flex().gap_3().children(
                    [
                        Control::ACT06,
                        Control::ACT07,
                        Control::ACT08,
                        Control::ACT09,
                    ]
                    .into_iter()
                    .map(|control| self.key(control, 98., cx)),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(px(98.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family("CaskaydiaMono Nerd Font")
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(23.))
                            .text_color(rgb(p.muted))
                            .child("W+"),
                    )
                    .child(self.key(Control::Mic, 208., cx))
                    .child(self.key(Control::ACT12, 98., cx)),
            )
    }

    pub(super) fn device_panel(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let mut dial_controls = div().flex().gap_2();
        for control in [
            Control::DialCounterclockwise,
            Control::DialPress,
            Control::DialClockwise,
        ] {
            let selected = self.selected == control;
            dial_controls = dial_controls.child(
                div()
                    .id(SharedString::from(format!("dial-{}", control.id())))
                    .px_3()
                    .py_2()
                    .text_size(px(11.))
                    .border_1()
                    .rounded(px(3.))
                    .border_color(rgb(if selected { p.accent } else { p.border }))
                    .text_color(rgb(if selected { p.accent } else { p.muted }))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.raised)))
                    .child(control.short_name())
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(control, window, cx)),
                    ),
            );
        }
        let mut body = div()
            .flex()
            .flex_col()
            .items_center()
            .gap_5()
            .py_8()
            .child(self.board(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.label("DIAL"))
                    .child(dial_controls),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(5.)).rounded_full().bg(rgb(p.accent)))
                    .child(self.label(if self.identify {
                        "Press a button, turn the dial, or move the joystick"
                    } else {
                        "Click a control to change its action"
                    })),
            )
            .child(
                div()
                    .w(px(468.))
                    .mt_3()
                    .border_t_1()
                    .border_color(rgb(p.border))
                    .pt_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.label("LIVE ACTIVITY")),
            );
        for activity in self.activity.iter().take(2) {
            body = body.child(
                div()
                    .w(px(468.))
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child(activity.clone()),
            );
        }
        if self.activity.is_empty() {
            body = body.child(
                div()
                    .w(px(468.))
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child("Device events and action results appear here."),
            );
        }
        let mut panel = div().flex_1().min_w_0().h_full().flex().flex_col().child(
            div()
                .h(px(62.))
                .px_7()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(self.icon("layers", 16., p.muted))
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(self.profiles.active().name.clone()),
                        )
                        .child(self.label(format!(
                            "  /  {} assigned",
                            self.profiles.active().bindings.len()
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(self.reset_backup.is_some(), |row| {
                            row.child(
                                div()
                                    .id("undo-reset")
                                    .px_3()
                                    .py_2()
                                    .text_size(px(11.))
                                    .text_color(rgb(p.accent))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(p.raised)))
                                    .child("Undo reset")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.undo_reset(window, cx)
                                    })),
                            )
                        })
                        .child(
                            div()
                                .id("reset-all")
                                .px_3()
                                .py_2()
                                .text_size(px(11.))
                                .text_color(rgb(if self.profiles.active().bindings.is_empty() {
                                    p.muted
                                } else {
                                    p.text
                                }))
                                .cursor_pointer()
                                .hover(|style| style.text_color(rgb(p.red)))
                                .child("Reset all")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.reset_all(window, cx)),
                                ),
                        )
                        .child(
                            div()
                                .id("identify")
                                .px_3()
                                .py_2()
                                .border_1()
                                .border_color(rgb(if self.identify { p.accent } else { p.border }))
                                .rounded(px(3.))
                                .text_size(px(11.))
                                .text_color(rgb(if self.identify { p.accent } else { p.text }))
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(p.raised)))
                                .child(if self.identify {
                                    "Listening…"
                                } else {
                                    "Identify key"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.identify = !this.identify;
                                    cx.notify();
                                })),
                        ),
                ),
        );
        if let DeviceStatus::Disconnected { message } = &self.status {
            panel = panel.child(
                div()
                    .mx_7()
                    .px_4()
                    .py_3()
                    .bg(rgb(p.raised))
                    .border_l_2()
                    .border_color(rgb(p.accent))
                    .text_size(px(12.))
                    .child(message.clone()),
            );
        }
        panel.child(
            div()
                .id("device-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(body),
        )
    }
}
