use super::CodexMicro;
use crate::{
    lighting::{Effect, Lighting},
    model::config_path,
};
use gpui::{
    Context, Div, Entity, SharedString, Stateful, Subscription, Window, div, prelude::*, px, rgb,
};
use gpui_component::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
};

pub(super) struct LightingEditor {
    pub page: bool,
    pub enabled: bool,
    pub draft: Lighting,
    pub target: usize,
    color: Entity<ColorPickerState>,
    brightness: Entity<SliderState>,
    speed: Entity<SliderState>,
}
impl LightingEditor {
    pub fn new(window: &mut Window, cx: &mut Context<CodexMicro>) -> (Self, Vec<Subscription>) {
        let light = crate::lighting::Light::default();
        let color = cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(light.color)));
        let brightness = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(f32::from(light.brightness))
        });
        let speed = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(f32::from(light.speed))
        });
        let subscriptions = vec![
            cx.subscribe(&color, |this, _, event: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(color)) = event {
                    this.lighting.draft.light_mut(this.lighting.target).color = picker_rgb(*color);
                    cx.notify();
                }
            }),
            cx.subscribe(&brightness, |this, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event;
                this.lighting
                    .draft
                    .light_mut(this.lighting.target)
                    .brightness = value.start().round() as u8;
                cx.notify();
            }),
            cx.subscribe(&speed, |this, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event;
                this.lighting.draft.light_mut(this.lighting.target).speed =
                    value.start().round() as u8;
                cx.notify();
            }),
        ];
        (
            Self {
                page: false,
                enabled: false,
                draft: Lighting::default(),
                target: 7,
                color,
                brightness,
                speed,
            },
            subscriptions,
        )
    }
}
const TARGETS: [&str; 8] = [
    "Agent 01",
    "Agent 02",
    "Agent 03",
    "Agent 04",
    "Agent 05",
    "Agent 06",
    "Command keys",
    "Border",
];

impl CodexMicro {
    pub(super) fn show_lighting(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shortcut_capture.reset();
        self.lighting.page = open;
        window.focus(&self.focus);
        cx.notify();
    }
    pub(super) fn load_lighting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.profiles.active().lighting.clone();
        self.lighting.enabled = saved.is_some();
        self.lighting.draft = saved.unwrap_or_default();
        self.select_light(self.lighting.target, window, cx);
    }
    pub(super) fn select_light(
        &mut self,
        target: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.lighting.target = target;
        let light = *self.lighting.draft.light(target);
        self.lighting.color.update(cx, |state, cx| {
            state.set_value(rgb(light.color), window, cx)
        });
        self.lighting.brightness.update(cx, |state, cx| {
            state.set_value(f32::from(light.brightness), window, cx)
        });
        self.lighting.speed.update(cx, |state, cx| {
            state.set_value(f32::from(light.speed), window, cx)
        });
        cx.notify();
    }
    pub(super) fn lighting_dirty(&self) -> bool {
        let draft = self.lighting.enabled.then_some(&self.lighting.draft);
        draft != self.profiles.active().lighting.as_ref()
    }
    pub(super) fn save_lighting(&mut self, cx: &mut Context<Self>) {
        if self.profile_dialog {
            return;
        }
        let mut updated = self.profiles.clone();
        updated.active_mut().lighting = self.lighting.enabled.then(|| self.lighting.draft.clone());
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.message = if self.lighting.enabled { "Lighting saved. Applies when connected on the Codex layer." } else { "Lighting control released. The device keeps its current colors until another app updates them." }.into();
                self.message_error = false;
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn lighting_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette;
        let light = self.lighting.draft.light(self.lighting.target);
        let enabled = self.lighting.enabled;
        let mut targets = div().flex().flex_wrap().gap_2();
        for (index, name) in TARGETS.into_iter().enumerate() {
            let selected = self.lighting.target == index;
            let setting = self.lighting.draft.light(index);
            targets =
                targets.child(
                    div()
                        .id(("light-target", index))
                        .px_3()
                        .py_2()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(rgb(if selected { p.accent } else { p.border }))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().size(px(8.)).rounded_full().bg(rgb(
                            if enabled && setting.effect != Effect::Off && setting.brightness > 0 {
                                setting.color
                            } else {
                                p.border
                            },
                        )))
                        .child(name)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_light(index, window, cx)
                        })),
                );
        }
        let mut effects = div().flex().flex_wrap().gap_2();
        for effect in Effect::ALL {
            effects = effects.child(
                div()
                    .id(SharedString::from(format!(
                        "light-effect-{}",
                        effect.label()
                    )))
                    .px_3()
                    .py_2()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(rgb(if light.effect == effect {
                        p.accent
                    } else {
                        p.border
                    }))
                    .text_color(rgb(if enabled { p.text } else { p.muted }))
                    .when(enabled, |el| el.cursor_pointer())
                    .child(effect.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.lighting.enabled {
                            this.lighting.draft.light_mut(this.lighting.target).effect = effect;
                            cx.notify();
                        }
                    })),
            );
        }
        div()
            .id("lighting-panel")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px_8()
            .py_6()
            .flex()
            .flex_col()
            .gap_5()
            .child(div().text_size(px(22.)).child("Lighting"))
            .child(self.label(format!(
                "Colors and effects for {}",
                self.profiles.active().name
            )))
            .child(
                Switch::new("lighting-enabled")
                    .label("Customize this profile")
                    .checked(enabled)
                    .on_click(cx.listener(|this, value, _, cx| {
                        this.lighting.enabled = *value;
                        cx.notify();
                    })),
            )
            .child(self.label("Six Agent LEDs, grouped Command keys, and an independent border."))
            .child(targets)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.label("EFFECT"))
                    .child(effects),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(self.label("COLOR"))
                    .when(
                        enabled && !matches!(light.effect, Effect::Off | Effect::Rainbow),
                        |row| {
                            row.child(ColorPicker::new(&self.lighting.color))
                                .child(self.label(format!("#{:06X}", light.color)))
                        },
                    )
                    .when(
                        !enabled || matches!(light.effect, Effect::Off | Effect::Rainbow),
                        |row| row.child(self.label(format!("#{:06X}", light.color))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.label(format!("BRIGHTNESS  {}%", light.brightness)))
                    .child(
                        Slider::new(&self.lighting.brightness)
                            .disabled(!enabled || light.effect == Effect::Off),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(self.label(format!("SPEED  {}%", light.speed)))
                    .child(
                        Slider::new(&self.lighting.speed)
                            .disabled(!enabled || !light.effect.animated()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .id("save-lighting")
                            .px_4()
                            .py_2()
                            .rounded(px(4.))
                            .bg(rgb(p.accent))
                            .text_color(rgb(p.panel))
                            .cursor_pointer()
                            .child("Save lighting")
                            .on_click(cx.listener(|this, _, _, cx| this.save_lighting(cx))),
                    )
                    .child(
                        div()
                            .id("discard-lighting")
                            .px_4()
                            .py_2()
                            .border_1()
                            .border_color(rgb(p.border))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .child("Discard changes")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.load_lighting(window, cx)),
                            ),
                    )
                    .child(self.label(if self.lighting_dirty() {
                        "Unsaved changes"
                    } else {
                        "Saved"
                    })),
            )
    }
}

fn picker_rgb(color: gpui::Hsla) -> u32 {
    let color = color.to_rgb();
    ((color.r * 255.).round() as u32) << 16
        | ((color.g * 255.).round() as u32) << 8
        | (color.b * 255.).round() as u32
}

#[cfg(test)]
mod tests {
    use gpui_component::Colorize;
    #[test]
    fn picker_preserves_exact_hex_channels() {
        for value in [
            0, 0xffffff, 0x123456, 0xff8800, 0x81a1c1, 0x010203, 0x2dd4bf,
        ] {
            let color = gpui::Hsla::parse_hex(&format!("#{value:06X}")).unwrap();
            assert_eq!(super::picker_rgb(color), value);
        }
    }
}
