use super::{CodexMicro, SaveBinding};
use crate::daemon::DeviceStatus;
use gpui::{
    Context, Div, FontWeight, IntoElement, Render, SharedString, Svg, Window, div, prelude::*, px,
    rgb, svg,
};
use gpui_component::{Root, Sizable, checkbox::Checkbox};

impl CodexMicro {
    pub(super) fn icon(&self, name: &str, size: f32, color: u32) -> Svg {
        svg()
            .path(format!("icons/{name}.svg"))
            .size(px(size))
            .text_color(rgb(color))
    }
    pub(super) fn label(&self, value: impl Into<SharedString>) -> Div {
        div()
            .text_size(px(11.))
            .text_color(rgb(self.palette.muted))
            .child(value.into())
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let (connected, subtitle) = match &self.status {
            DeviceStatus::Connected { transport, .. } => (true, transport.as_str()),
            DeviceStatus::Connecting => (false, "Connecting"),
            DeviceStatus::Disconnected { .. } => (false, "Disconnected"),
        };
        let mut profiles = div()
            .id("profiles-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pb_3();
        for (index, profile) in self.profiles.list().iter().enumerate() {
            let active = index == self.profiles.active_index();
            profiles = profiles.child(
                div()
                    .id(("profile", index))
                    .mx_3()
                    .px_3()
                    .py_3()
                    .flex_shrink_0()
                    .rounded(px(3.))
                    .when(active, |row| row.bg(rgb(p.raised)))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.raised)))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(self.icon("layers", 18., if active { p.accent } else { p.muted }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .child(profile.name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .border_1()
                            .border_color(rgb(if active { p.accent } else { p.border }))
                            .rounded(px(3.))
                            .child(
                                Checkbox::new(("active-profile", index))
                                    .small()
                                    .checked(active)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let _ = this.activate_profile(index, window, cx);
                                    })),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let _ = this.activate_profile(index, window, cx);
                    })),
            );
        }
        profiles = profiles.child(
            div()
                .id("add-profile")
                .mx_3()
                .mt_2()
                .px_3()
                .py_3()
                .flex_shrink_0()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(p.border))
                .text_color(rgb(p.muted))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(p.raised)).text_color(rgb(p.text)))
                .flex()
                .gap_3()
                .items_center()
                .child(self.icon("plus", 16., p.muted))
                .child(div().text_size(px(12.)).child("Add profile"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_profile_dialog(window, cx);
                })),
        );
        div()
            .w(px(196.))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(p.panel))
            .border_r_1()
            .border_color(rgb(p.border))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(88.))
                    .px_5()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(self.icon("device", 27., p.accent))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("CODEX MICRO"),
                            )
                            .child(self.label("DEVICE CONTROLS")),
                    ),
            )
            .child(div().px_5().pt_5().pb_3().child(self.label("DEVICES")))
            .child(
                div()
                    .mx_3()
                    .px_3()
                    .py_3()
                    .bg(rgb(p.raised))
                    .border_l_2()
                    .border_color(rgb(p.accent))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(self.icon("device", 21., p.accent))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(13.)).child("Codex Micro"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().size(px(5.)).rounded_full().bg(rgb(if connected {
                                            p.green
                                        } else {
                                            p.muted
                                        })),
                                    )
                                    .child(self.label(subtitle.to_owned())),
                            ),
                    ),
            )
            .child(div().px_5().pt_8().pb_3().child(self.label("PROFILES")))
            .child(profiles)
    }
    fn page_tabs(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let mut row = div()
            .px_8()
            .py_2()
            .flex()
            .gap_4()
            .border_b_1()
            .border_color(rgb(p.border));
        for (page, name) in [(false, "Bindings"), (true, "Lighting")] {
            row =
                row.child(
                    div()
                        .id(name)
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .text_color(rgb(if self.lighting.page == page {
                            p.accent
                        } else {
                            p.muted
                        }))
                        .child(name)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.show_lighting(page, window, cx)
                        })),
                );
        }
        row
    }
    fn header(&self) -> Div {
        let p = self.palette;
        let status = match &self.status {
            DeviceStatus::Connected {
                transport,
                battery,
                charging,
                ..
            } => format!(
                "{transport}   ·   {battery}%{}",
                if *charging { "  charging" } else { "" }
            ),
            DeviceStatus::Connecting => "Connecting to your Micro".into(),
            DeviceStatus::Disconnected { .. } => "Device disconnected".into(),
        };
        div()
            .h(px(104.))
            .flex_shrink_0()
            .px_8()
            .flex()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(27.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Codex Micro"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(p.muted))
                            .child("Button assignments and lighting for your Omarchy desktop."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(self.icon("battery", 18., p.muted))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.muted))
                            .child(status),
                    ),
            )
    }
}

impl Render for CodexMicro {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let dialog_layer = Root::render_dialog_layer(window, cx);
        div()
            .size_full()
            .bg(rgb(p.background))
            .text_color(rgb(p.text))
            .font_family("Inter")
            .text_size(px(14.))
            .flex()
            .track_focus(&self.focus)
            .key_context("CodexMicro")
            .capture_key_up(cx.listener(Self::shortcut_key_up))
            .on_action(cx.listener(|this, _: &SaveBinding, window, cx| this.save(window, cx)))
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.header())
                    .child(self.page_tabs(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.device_panel(cx))
                            .when(self.lighting.page, |panel| {
                                panel.child(self.lighting_panel(cx))
                            })
                            .when(!self.lighting.page, |panel| {
                                panel.child(self.editor(window, cx))
                            }),
                    )
                    .child(
                        div()
                            .h(px(38.))
                            .flex_shrink_0()
                            .px_5()
                            .border_t_1()
                            .border_color(rgb(p.border))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.icon(
                                if self.message_error { "x" } else { "check" },
                                13.,
                                if self.message_error { p.red } else { p.muted },
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(rgb(if self.message_error {
                                        p.red
                                    } else {
                                        p.muted
                                    }))
                                    .child(self.message.clone()),
                            )
                            .child(self.label("Saved settings stay active when closed")),
                    ),
            )
            .children(dialog_layer)
    }
}
