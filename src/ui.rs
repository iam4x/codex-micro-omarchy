mod bindings;
mod control;
mod device_view;
mod editor;
mod events;
mod layout;
mod shortcut;

actions!(codex_micro, [SaveBinding]);

use crate::{
    daemon::{DeviceStatus, Event},
    model::{Control, Phase, Preset, Profile, config_path},
    theme::{Assets, Palette},
};
use gpui::{
    App, AppContext, Application, Bounds, Context, Entity, FocusHandle, KeyBinding, KeyDownEvent,
    ScrollHandle, Subscription, TitlebarOptions, Window, WindowBounds, WindowOptions, actions,
    point, px, size,
};
use gpui_component::{
    Root,
    input::{InputEvent, InputState},
};
use std::{collections::VecDeque, time::Instant};

use bindings::ActionTab;

pub(super) struct CodexMicro {
    palette: Palette,
    profile: Profile,
    selected: Control,
    phase: Phase,
    tab: ActionTab,
    preset: Preset,
    input: Entity<InputState>,
    text_input: Entity<InputState>,
    text_submit: bool,
    action_scroll: ScrollHandle,
    action_search: Entity<InputState>,
    status: DeviceStatus,
    message: String,
    message_error: bool,
    identify: bool,
    shortcut_focus: FocusHandle,
    shortcut_capture: shortcut::ShortcutCapture,
    reset_backup: Option<Profile>,
    active: Option<(Control, Instant)>,
    activity: VecDeque<String>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl CodexMicro {
    fn new(profile: Profile, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = Palette::load();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter an action"));
        let subscription = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let text_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter text…")
                .multi_line(true)
                .rows(6)
                .soft_wrap(true)
        });
        let text_subscription = cx.subscribe(&text_input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let action_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search actions…"));
        let search_subscription = cx.subscribe(&action_search, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.action_scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
        });
        let recorder = cx.weak_entity();
        let intercept_subscription = cx.intercept_keystrokes(move |event, window, cx| {
            let _ = recorder.update(cx, |this, cx| {
                this.shortcut_key_down(
                    &KeyDownEvent {
                        keystroke: event.keystroke.clone(),
                        is_held: false,
                    },
                    window,
                    cx,
                );
            });
        });
        let shortcut_focus = cx.focus_handle();
        let blur_subscription = cx.on_blur(&shortcut_focus, window, |this, _, cx| {
            this.shortcut_capture.reset();
            cx.notify();
        });
        let activation_subscription = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.shortcut_capture.is_held() {
                this.shortcut_capture.reset();
                cx.notify();
            }
        });
        Self::start_events(window, cx);
        let mut view = Self {
            palette,
            profile,
            selected: Control::AG00,
            phase: Phase::Press,
            tab: ActionTab::System,
            preset: Preset::Terminal,
            input,
            text_input,
            text_submit: false,
            action_scroll: ScrollHandle::new(),
            action_search,
            status: DeviceStatus::Connecting,
            message: "Select a control to get started".into(),
            message_error: false,
            identify: false,
            shortcut_focus,
            shortcut_capture: shortcut::ShortcutCapture::default(),
            reset_backup: None,
            active: None,
            activity: VecDeque::new(),
            focus: cx.focus_handle(),
            _subscriptions: vec![
                subscription,
                text_subscription,
                search_subscription,
                blur_subscription,
                activation_subscription,
                intercept_subscription,
            ],
        };
        view.load_editor(window, cx);
        view
    }

    fn receive(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Status(status) => self.status = status,
            Event::Input(input) => {
                self.active = Some((input.control, Instant::now()));
                if self.identify && input.phase != Phase::Release {
                    self.select(input.control, window, cx);
                    self.identify = false;
                    self.message = format!("Selected {}", input.control.name());
                    self.message_error = false;
                }
                self.push_activity(format!(
                    "{} · {}",
                    input.control.name(),
                    input.phase.label()
                ));
            }
            Event::Action {
                title,
                message,
                success,
            } => {
                self.push_activity(format!("{title} · {message}"));
                if !success {
                    self.message = message;
                    self.message_error = true;
                }
            }
            Event::ConfigError(error) => {
                self.message = error;
                self.message_error = true;
            }
        }
    }
    fn push_activity(&mut self, text: String) {
        self.activity.push_front(text);
        self.activity.truncate(4);
    }
}

pub fn run() -> anyhow::Result<()> {
    use fs2::FileExt;
    let lock_path = config_path().with_extension("ui.lock");
    std::fs::create_dir_all(
        lock_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid runtime directory"))?,
    )?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    if let Err(error) = lock.try_lock_exclusive() {
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(error.into());
        }
        std::process::Command::new("hyprctl")
            .args(["dispatch", "hl.dsp.focus({window=\"class:codex-micro\"})"])
            .status()?;
        return Ok(());
    }
    let profile = Profile::load(&config_path())?;
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys([KeyBinding::new("ctrl-s", SaveBinding, Some("CodexMicro"))]);
            Palette::load().apply(cx);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1240.), px(860.)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1120.), px(760.))),
                    app_id: Some("codex-micro".into()),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Codex Micro".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_window_title("Codex Micro");
                    let view = cx.new(|cx| CodexMicro::new(profile, window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            if let Err(error) = result {
                eprintln!("Cannot open Codex Micro: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}
