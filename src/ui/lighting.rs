use super::CodexMicro;
use crate::{
    lighting::{Effect, Lighting},
    model::config_path,
};
use gpui::{
    Context, Div, Entity, MouseButton, SharedString, Stateful, Subscription, Window, div,
    prelude::*, px, rgb,
};
use gpui_component::{
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    slider::{Slider, SliderEvent, SliderState},
};

pub(super) struct LightingEditor {
    pub page: bool,
    pub settings: Lighting,
    pub target: Option<usize>,
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
                .step(10.)
                .default_value(f32::from(light.brightness))
        });
        let speed = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(10.)
                .default_value(f32::from(light.speed))
        });
        let subscriptions = vec![
            cx.subscribe(&color, |this, _, event: &ColorPickerEvent, cx| {
                if let ColorPickerEvent::Change(Some(color)) = event
                    && let Some(target) = this.lighting.target
                {
                    this.lighting.settings.light_mut(target).color = picker_rgb(*color);
                    this.apply_lighting(cx);
                }
            }),
            cx.subscribe_in(
                &brightness,
                window,
                |this, slider, event: &SliderEvent, window, cx| {
                    let Some(target) = this.lighting.target else {
                        return;
                    };
                    let SliderEvent::Change(value) = event;
                    this.lighting.settings.light_mut(target).brightness =
                        snap_percentage(value.start());
                    slider.update(cx, |state, cx| {
                        state.set_value(f32::from(snap_percentage(value.start())), window, cx)
                    });
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &speed,
                window,
                |this, slider, event: &SliderEvent, window, cx| {
                    let Some(target) = this.lighting.target else {
                        return;
                    };
                    let SliderEvent::Change(value) = event;
                    this.lighting.settings.light_mut(target).speed = snap_percentage(value.start());
                    slider.update(cx, |state, cx| {
                        state.set_value(f32::from(snap_percentage(value.start())), window, cx)
                    });
                    cx.notify();
                },
            ),
        ];
        (
            Self {
                page: false,
                settings: Lighting::default(),
                target: None,
                color,
                brightness,
                speed,
            },
            subscriptions,
        )
    }
}
const TARGETS: [&str; 8] = [
    "Key 01",
    "Key 02",
    "Key 03",
    "Key 04",
    "Key 05",
    "Key 06",
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
        self.lighting.settings = self.profiles.active().lighting.clone();
        if let Some(target) = self.lighting.target {
            self.select_light(target, window, cx);
        }
    }
    pub(super) fn select_light(
        &mut self,
        target: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.lighting.target = Some(target);
        let light = *self.lighting.settings.light(target);
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
    pub(super) fn apply_lighting(&mut self, cx: &mut Context<Self>) {
        let mut updated = self.profiles.clone();
        updated.active_mut().lighting = self.lighting.settings.clone();
        match updated.save(&config_path()) {
            Ok(()) => {
                self.profiles = updated;
                self.message = "Lighting updated".into();
                self.message_error = false;
            }
            Err(error) => {
                self.message = format!("{error:#}");
                self.message_error = true;
            }
        }
        cx.notify();
    }
    pub(super) fn slider_change(&self, speed: bool, value: u8, cx: &mut Context<Self>) {
        let slider = if speed {
            &self.lighting.speed
        } else {
            &self.lighting.brightness
        };
        slider.update(cx, |_, cx| {
            cx.emit(SliderEvent::Change(f32::from(value).into()))
        });
    }
    pub(super) fn finish_lighting_drag(&mut self, cx: &mut Context<Self>) {
        if self.lighting.settings != self.profiles.active().lighting {
            self.apply_lighting(cx);
        }
    }
    fn lighting_slider(&self, state: &Entity<SliderState>) -> Div {
        let mut ticks = div().flex().justify_between().w_full();
        for _ in 0..=10 {
            ticks = ticks.child(div().w(px(1.)).h(px(6.)).bg(rgb(self.palette.muted)));
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Slider::new(state))
            .child(ticks)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(self.label("0%"))
                    .child(self.label("100%")),
            )
    }
    pub(super) fn lighting_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette;
        let mut targets = div().flex().flex_wrap().gap_2();
        for (index, name) in TARGETS.into_iter().enumerate() {
            let selected = self.lighting.target == Some(index);
            let setting = self.lighting.settings.light(index);
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
                            if setting.effect != Effect::Off && setting.brightness > 0 {
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
        let panel =
            div()
                .id("lighting-panel")
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.finish_lighting_drag(cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.finish_lighting_drag(cx)),
                )
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
                .child(self.label(
                    "Six individual key LEDs, grouped Command keys, and an independent border.",
                ))
                .child(targets);
        let Some(target) = self.lighting.target else {
            return panel.child(self.label("Select a key or lighting zone to customize it."));
        };
        let light = self.lighting.settings.light(target);
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
                    .text_color(rgb(p.text))
                    .cursor_pointer()
                    .child(effect.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let Some(target) = this.lighting.target else {
                            return;
                        };
                        this.lighting.settings.light_mut(target).effect = effect;
                        this.apply_lighting(cx);
                    })),
            );
        }
        panel
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
                        !matches!(light.effect, Effect::Off | Effect::Rainbow),
                        |row| {
                            row.child(ColorPicker::new(&self.lighting.color))
                                .child(self.label(format!("#{:06X}", light.color)))
                        },
                    )
                    .when(
                        matches!(light.effect, Effect::Off | Effect::Rainbow),
                        |row| row.child(self.label(format!("#{:06X}", light.color))),
                    ),
            )
            .when(light.effect != Effect::Off, |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(self.label(format!("BRIGHTNESS  {}%", light.brightness)))
                        .child(self.lighting_slider(&self.lighting.brightness)),
                )
            })
            .when(light.effect.animated(), |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(self.label(format!("SPEED  {}%", light.speed)))
                        .child(self.lighting_slider(&self.lighting.speed)),
                )
            })
    }
}

fn snap_percentage(value: f32) -> u8 {
    ((value.clamp(0., 100.) / 10.).round() * 10.) as u8
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
