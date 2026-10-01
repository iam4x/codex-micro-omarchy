mod ai;
mod bindings;
mod control;
mod device_view;
mod editor;
mod events;
mod layout;
mod lighting;
mod profiles;
mod shortcut;

actions!(codex_micro, [SaveBinding]);

use crate::{
    daemon::{DeviceStatus, Event},
    model::{Control, Phase, Preset, Profile, Profiles, config_path},
    theme::{Assets, Palette},
};
use gpui::{
    App, AppContext, Application, Bounds, Context, Entity, FocusHandle, KeyBinding, KeyDownEvent,
    ScrollHandle, Subscription, TitlebarOptions, Window, WindowBounds, WindowOptions, actions,
    point, px, size,
};
use gpui_component::{
    Root,
    input::{Enter as InputEnter, InputEvent, InputState},
};
use std::{collections::VecDeque, time::Instant};

use bindings::ActionTab;

pub(super) struct CodexMicro {
    palette: Palette,
    lighting: lighting::LightingEditor,
    profiles: Profiles,
    profile_dialog: bool,
    profile_name: Entity<InputState>,
    profile_error: Entity<Option<String>>,
    selected: Control,
    phase: Phase,
    tab: ActionTab,
    preset: Preset,
    input: Entity<InputState>,
    text_input: Entity<InputState>,
    text_submit: bool,
    ai_prompt: Entity<InputState>,
    ai_state: ai::AiState,
    ai_revision: u64,
    action_scroll: ScrollHandle,
    action_search: Entity<InputState>,
    status: DeviceStatus,
    message: String,
    message_error: bool,
    identify: bool,
    shortcut_manual: bool,
    shortcut_focus: FocusHandle,
    shortcut_capture: shortcut::ShortcutCapture,
    reset_backup: Option<Profile>,
    active: Option<(Control, Instant)>,
    activity: VecDeque<String>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl CodexMicro {
    fn new(profiles: Profiles, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        let ai_prompt = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("For example: Focus T3 Code, or launch it if it is closed…")
                .multi_line(true)
                .rows(5)
                .soft_wrap(true)
        });
        let ai_subscription = cx.subscribe(&ai_prompt, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.ai_prompt_changed(cx);
            }
        });
        let ai_quit_subscription = cx.on_app_quit(|this, _| {
            this.invalidate_ai();
            async {}
        });
        let ai_release_subscription = cx.on_release(|this, _| this.invalidate_ai());
        let action_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search actions…"));
        let search_subscription = cx.subscribe(&action_search, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.action_scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
        });
        let profile_name = cx.new(|cx| InputState::new(window, cx).placeholder("Profile name"));
        let profile_error = cx.new(|_| None);
        let profile_subscription =
            cx.subscribe_in(&profile_name, window, |this, _, event, window, cx| {
                if !this.profile_dialog {
                    return;
                }
                match event {
                    InputEvent::Change => {
                        this.profile_error.update(cx, |error, _| *error = None);
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.submit_profile(window, cx),
                    _ => {}
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
        let (lighting, lighting_subscriptions) = lighting::LightingEditor::new(window, cx);
        let mut view = Self {
            palette,
            lighting,
            profiles,
            profile_dialog: false,
            profile_name,
            profile_error,
            selected: Control::AG00,
            phase: Phase::Press,
            tab: ActionTab::System,
            preset: Preset::Terminal,
            input,
            text_input,
            text_submit: false,
            ai_prompt,
            ai_state: ai::AiState::Idle,
            ai_revision: 0,
            action_scroll: ScrollHandle::new(),
            action_search,
            status: DeviceStatus::Connecting,
            message: "Select a control to get started".into(),
            message_error: false,
            identify: false,
            shortcut_manual: false,
            shortcut_focus,
            shortcut_capture: shortcut::ShortcutCapture::default(),
            reset_backup: None,
            active: None,
            activity: VecDeque::new(),
            focus: cx.focus_handle(),
            _subscriptions: vec![
                subscription,
                profile_subscription,
                text_subscription,
                ai_subscription,
                ai_quit_subscription,
                ai_release_subscription,
                search_subscription,
                blur_subscription,
                activation_subscription,
                intercept_subscription,
            ],
        };
        view._subscriptions.extend(lighting_subscriptions);
        view.load_lighting(window, cx);
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
            Event::LightingApplied => {
                self.message = "Lighting applied to your Micro".into();
                self.message_error = false;
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
    let profiles = Profiles::load(&config_path())?;
    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys([
                KeyBinding::new("ctrl-s", SaveBinding, Some("CodexMicro")),
                KeyBinding::new(
                    "shift-enter",
                    InputEnter { secondary: false },
                    Some("Input"),
                ),
            ]);
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
                    let view = cx.new(|cx| CodexMicro::new(profiles, window, cx));
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
