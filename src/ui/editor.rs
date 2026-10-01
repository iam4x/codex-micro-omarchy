use super::{ActionTab, CodexMicro};
use crate::model::{Action, Phase, Preset};
use gpui::{
    App, Context, Div, FontWeight, Rgba, SharedString, Window, div, prelude::*, px, rgb, rgba,
};
use gpui_component::{Sizable, input::Input, switch::Switch};

impl CodexMicro {
    pub(super) fn visible_presets(&self, cx: &App) -> Vec<Preset> {
        let query = self.action_search.read(cx).value().trim().to_lowercase();
        Preset::ALL
            .into_iter()
            .filter(|preset| {
                preset.name().to_lowercase().contains(&query)
                    || preset.description().to_lowercase().contains(&query)
            })
            .collect()
    }

    pub(super) fn editor(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let assigned = self.profiles.active().action(self.selected, self.phase);
        let mut tabs = div().flex().flex_col().gap_1().mb_5();
        for row in ActionTab::ALL.chunks(3) {
            let mut tab_row = div().flex().gap_1();
            for &tab in row {
                let selected = self.tab == tab;
                tab_row = tab_row.child(
                    div()
                        .id(SharedString::from(format!("tab-{}", tab.name())))
                        .flex_1()
                        .py_2()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(rgb(if selected { p.accent } else { p.border }))
                        .text_color(rgb(if selected { p.accent } else { p.muted }))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.raised)))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .items_center()
                        .child(self.icon(
                            tab.icon(),
                            16.,
                            if selected { p.accent } else { p.muted },
                        ))
                        .child(div().text_size(px(10.)).child(tab.name()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.switch_tab(tab, window, cx);
                        })),
                );
            }
            tabs = tabs.child(tab_row);
        }
        let mut action_form = div().w_full().flex_shrink_0().flex().flex_col().gap_2();
        if self.tab == ActionTab::System {
            let presets = self.visible_presets(cx);
            if presets.is_empty() {
                action_form = action_form.child(
                    div()
                        .py_5()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_size(px(12.)).child("No actions found"))
                        .child(self.label("Try a different search.")),
                );
            }
            for preset in presets {
                let selected = self.preset == preset;
                action_form = action_form.child(
                    div()
                        .id(SharedString::from(format!("preset-{}", preset.name())))
                        .flex_shrink_0()
                        .px_3()
                        .py_3()
                        .border_1()
                        .border_color(rgb(if selected { p.accent } else { p.border }))
                        .rounded(px(3.))
                        .bg(rgb(if selected { 0x202a33 } else { p.panel }))
                        .cursor_pointer()
                        .hover(|style| style.border_color(rgb(p.accent)))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(self.icon(
                            preset.icon(),
                            19.,
                            if selected { p.accent } else { p.muted },
                        ))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().text_size(px(12.)).child(preset.name()))
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(rgb(p.muted))
                                        .child(preset.description()),
                                ),
                        )
                        .when(selected, |item| {
                            item.child(self.icon("check", 15., p.accent))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.preset = preset;
                            cx.notify();
                        })),
                );
            }
        } else if self.tab == ActionTab::Ai {
            action_form = self.ai_editor(window, cx);
        } else {
            let (label, help, example) = match self.tab {
                ActionTab::App => (
                    "APPLICATION COMMAND",
                    "Enter an application name or an executable path. Arguments and quoted paths are supported.",
                    "firefox",
                ),
                ActionTab::Shortcut if self.shortcut_manual => (
                    "KEYBOARD SHORTCUT",
                    "Type a shortcut such as Super+Left. Use this for shortcuts that your desktop intercepts.",
                    "Super+Left",
                ),
                ActionTab::Shortcut => (
                    "KEYBOARD SHORTCUT",
                    "Press your shortcut in the field. Release all keys to capture it, including Esc. Use × to clear.",
                    "Ctrl+Shift+C",
                ),
                ActionTab::Text => (
                    "TEXT TO TYPE",
                    "Press Enter for a new line. Line breaks are typed as Shift+Enter in the focused application. Supports Unicode.",
                    "Your text here",
                ),
                ActionTab::Command => (
                    "SHELL COMMAND",
                    "Run a command or script with sh. Pipes, variables, and shell syntax are supported.",
                    "notify-send 'Hello from my Micro'",
                ),
                ActionTab::System | ActionTab::Ai => unreachable!(),
            };
            let input = if self.tab == ActionTab::Shortcut && !self.shortcut_manual {
                let focused = self.shortcut_focus.is_focused(window);
                let value = self
                    .shortcut_capture
                    .preview()
                    .unwrap_or_else(|| self.input.read(cx).value().to_string());
                div()
                    .id("shortcut-input")
                    .track_focus(&self.shortcut_focus)
                    .tab_index(0)
                    .w_full()
                    .h(px(36.))
                    .px_3()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(rgb(if focused { p.accent } else { p.border }))
                    .bg(rgb(p.background))
                    .text_size(px(12.))
                    .text_color(rgb(if value.is_empty() { p.muted } else { p.text }))
                    .cursor_text()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(if value.is_empty() {
                                "Press a shortcut".into()
                            } else {
                                value
                            }),
                    )
                    .child(
                        div()
                            .id("clear-shortcut")
                            .size(px(24.))
                            .flex_shrink_0()
                            .rounded(px(3.))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(p.raised)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(self.icon("x", 14., p.muted))
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.clear_shortcut(window, cx);
                            })),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.shortcut_capture.reset();
                        window.focus(&this.shortcut_focus);
                        cx.notify();
                    }))
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.shortcut_capture.reset();
                        cx.notify();
                    }))
                    .on_modifiers_changed(cx.listener(Self::shortcut_modifiers))
                    .into_any_element()
            } else if self.tab == ActionTab::Text {
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .child(Input::new(&self.text_input).h(px(160.)).flex_1().min_w_0())
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .child(
                        Input::new(&self.input)
                            .cleanable(self.tab == ActionTab::Shortcut)
                            .flex_1()
                            .min_w_0(),
                    )
                    .into_any_element()
            };
            action_form = action_form.child(self.label(label));
            if self.tab == ActionTab::Shortcut {
                action_form = action_form.child(
                    Switch::new("shortcut-manual")
                        .small()
                        .checked(self.shortcut_manual)
                        .label("Type shortcut")
                        .on_click(cx.listener(|this, manual, window, cx| {
                            this.set_shortcut_manual(*manual, window, cx);
                        })),
                );
            }
            action_form = action_form.child(input);
            if self.tab == ActionTab::Text {
                action_form = action_form.child(
                    div().mt_2().child(
                        Switch::new("text-submit")
                            .small()
                            .checked(self.text_submit)
                            .label("Submit after typing")
                            .tooltip("Press Enter after the full text has been typed")
                            .on_click(cx.listener(|this, enabled, _, cx| {
                                this.set_text_submit(*enabled, cx);
                            })),
                    ),
                );
            }
            action_form = action_form
                .child(
                    div()
                        .mt_2()
                        .text_size(px(12.))
                        .text_color(rgb(p.muted))
                        .child(help),
                )
                .when(self.tab != ActionTab::Text, |form| {
                    form.child(
                        div()
                            .mt_3()
                            .p_3()
                            .bg(rgb(p.raised))
                            .rounded(px(3.))
                            .text_size(px(11.))
                            .font_family("CaskaydiaMono Nerd Font")
                            .child(example),
                    )
                });
        }
        let mut phase_selector = div().flex().gap_2().mb_5();
        if !self.selected.is_rotation() {
            for phase in [Phase::Press, Phase::Release] {
                phase_selector = phase_selector.child(
                    div()
                        .id(SharedString::from(format!("phase-{phase:?}")))
                        .flex_1()
                        .py_2()
                        .text_center()
                        .text_size(px(11.))
                        .border_b_2()
                        .border_color(rgb(if self.phase == phase {
                            p.accent
                        } else {
                            p.border
                        }))
                        .text_color(rgb(if self.phase == phase { p.text } else { p.muted }))
                        .cursor_pointer()
                        .child(phase.label())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_phase(phase, cx);
                        })),
                );
            }
        } else {
            phase_selector = phase_selector.child(self.label("ONE ACTION PER DIAL STEP"));
        }
        let header = div()
            .flex_shrink_0()
            .px_5()
            .pt_5()
            .flex()
            .flex_col()
            .child(self.label("SELECTED CONTROL"))
            .child(
                div()
                    .mt_2()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.selected.name()),
            )
            .child(
                div()
                    .mt_1()
                    .mb_5()
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child(format!(
                        "{}  ·  {}",
                        self.selected.id(),
                        assigned
                            .map(Action::title)
                            .unwrap_or_else(|| "No action assigned".into())
                    )),
            )
            .child(phase_selector)
            .child(self.label("CHOOSE AN ACTION"))
            .child(div().h(px(12.)))
            .child(tabs)
            .when(self.tab == ActionTab::System, |header| {
                header.child(
                    div().pb_4().child(
                        Input::new(&self.action_search)
                            .h(px(36.))
                            .prefix(self.icon("search", 16., p.muted))
                            .cleanable(true),
                    ),
                )
            });
        div()
            .w(px(336.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(p.panel))
            .border_l_1()
            .border_color(rgb(p.border))
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("action-list-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.action_scroll)
                    .px_5()
                    .pb_5()
                    .child(action_form),
            )
            .child(self.footer(assigned.is_some(), cx))
    }
}

impl CodexMicro {
    fn footer(&self, assigned: bool, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let dirty = self.is_dirty(cx);
        let action_ready = self.tab != ActionTab::Ai || self.ai_can_run(cx);
        let (dot, state, hint) = match (assigned, dirty) {
            (true, false) => (p.green, "Saved", "Active on your device"),
            (true, true) => (p.accent, "Unsaved changes", "Ctrl+S to save"),
            (false, _) => (p.muted, "Not assigned", "Ctrl+S to assign"),
        };
        let button = |id: &'static str, enabled: bool, hover: Rgba| {
            div()
                .id(id)
                .h(px(33.))
                .px_3()
                .border_1()
                .border_color(rgb(p.border))
                .rounded(px(3.))
                .text_size(px(11.))
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(rgb(if enabled { p.text } else { p.muted }))
                .when(enabled, |button| {
                    button.cursor_pointer().hover(move |style| style.bg(hover))
                })
                .when(!enabled, |button| button.opacity(0.5))
        };
        div()
            .flex_shrink_0()
            .p_5()
            .border_t_1()
            .border_color(rgb(p.border))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(6.)).rounded_full().bg(rgb(dot)))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(if dirty && assigned { p.text } else { p.muted }))
                            .child(state),
                    )
                    .child(div().flex_1())
                    .child(self.label(hint)),
            )
            .child(
                div()
                    .id("save-binding")
                    .h(px(42.))
                    .rounded(px(3.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .when(!action_ready, |button| button.opacity(0.45))
                    .map(|button| {
                        if dirty {
                            button
                                .bg(rgb(p.accent))
                                .text_color(rgb(p.panel))
                                .when(action_ready, |button| {
                                    button.cursor_pointer().hover(|style| style.opacity(0.85))
                                })
                                .child(self.icon("check", 16., p.panel))
                                .child(if assigned {
                                    "Save changes"
                                } else {
                                    "Assign action"
                                })
                                .when(action_ready, |button| {
                                    button.on_click(
                                        cx.listener(|this, _, window, cx| this.save(window, cx)),
                                    )
                                })
                        } else {
                            button
                                .border_1()
                                .border_color(rgb(p.border))
                                .text_color(rgb(p.muted))
                                .child(self.icon("check", 16., p.green))
                                .child("Saved")
                        }
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        button("test-action", action_ready, rgb(p.raised))
                            .flex_1()
                            .child(self.icon("play", 12., p.muted))
                            .child("Test action")
                            .when(action_ready, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| this.test(cx)))
                            }),
                    )
                    .child(
                        button("discard-changes", dirty && assigned, rgb(p.raised))
                            .flex_1()
                            .child("Discard changes")
                            .when(dirty && assigned, |button| {
                                button.on_click(
                                    cx.listener(|this, _, window, cx| this.discard(window, cx)),
                                )
                            }),
                    ),
            )
            .child(
                button("remove-binding", assigned, rgba(p.red << 8 | 0x1a))
                    .border_color(if assigned {
                        rgba(p.red << 8 | 0x55)
                    } else {
                        rgba(p.border << 8 | 0xff)
                    })
                    .text_color(rgb(if assigned { p.red } else { p.muted }))
                    .child(self.icon("circle-x", 13., if assigned { p.red } else { p.muted }))
                    .child("Remove binding")
                    .when(assigned, |button| {
                        button.on_click(
                            cx.listener(|this, _, window, cx| this.confirm_remove(window, cx)),
                        )
                    }),
            )
    }
}
