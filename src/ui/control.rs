//! Opt-in automation for the real native window. Enabled only by a local
//! CODEX_MICRO_CONTROL_SOCKET path, with access restricted to the current user.
use super::{ActionTab, CodexMicro};
use crate::model::{Control, Phase, Preset};
use crate::wire;
use gpui::{
    App, AsyncWindowContext, Context, KeyUpEvent, Keystroke, Modifiers, ModifiersChangedEvent,
    WeakEntity, Window, point, px,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::BufReader,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InputCommand {
    Ui {
        #[serde(flatten)]
        action: UiAction,
    },
    Type {
        text: String,
    },
    Inspect,
    Key {
        key: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UiAction {
    Select { control: Control },
    Tab { tab: ActionTab },
    Phase { phase: Phase },
    Preset { preset: Preset },
    Scroll { y: f32 },
    FocusSearch,
    FocusInput,
    TextSubmit { enabled: bool },
    ClearShortcut,
    ShortcutKeyUp { key: String },
    ShortcutModifiers { modifiers: Modifiers },
    Save,
    Test,
    Remove,
    Reset,
    UndoReset,
    OpenProfileDialog,
    CancelProfileDialog,
    SubmitProfile,
    ActivateProfile { index: usize },
}
#[derive(Serialize)]
pub struct Reply {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Snapshot>,
}

#[derive(Serialize)]
struct Snapshot {
    query: String,
    actions: Vec<&'static str>,
    dirty: bool,
    phase: Phase,
    tab: &'static str,
    preset: Preset,
    scroll_y: f32,
    input: String,
    text_submit: bool,
    shortcut_preview: Option<String>,
    shortcut_held: bool,
    profiles: Vec<String>,
    active_profile: usize,
    profile_dialog: bool,
    profile_name: String,
    profile_error: Option<String>,
}

impl Reply {
    fn from_result(result: anyhow::Result<Option<Snapshot>>) -> Self {
        match result {
            Ok(state) => Self {
                ok: true,
                error: None,
                state,
            },
            Err(error) => Self {
                ok: false,
                error: Some(format!("{error:#}")),
                state: None,
            },
        }
    }
}

pub type PendingInput = (InputCommand, mpsc::SyncSender<Reply>);

pub fn receiver() -> anyhow::Result<Option<mpsc::Receiver<PendingInput>>> {
    let Some(path) = std::env::var_os("CODEX_MICRO_CONTROL_SOCKET") else {
        return Ok(None);
    };
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for client in listener.incoming() {
            let Ok(mut client) = client else {
                break;
            };
            if client
                .set_read_timeout(Some(Duration::from_secs(3)))
                .is_err()
                || client
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .is_err()
            {
                continue;
            }
            let command = match wire::read(&mut BufReader::new(&mut client)) {
                Ok(command) => command,
                Err(error) => {
                    let _ = wire::write(&mut client, &Reply::from_result(Err(error)));
                    continue;
                }
            };
            let (done, wait) = mpsc::sync_channel(1);
            if sender.send((command, done)).is_err() {
                break;
            }
            if let Ok(response) = wait.recv_timeout(Duration::from_secs(3)) {
                let _ = wire::write(&mut client, &response);
            }
        }
    });
    Ok(Some(receiver))
}

pub fn dispatch(
    command: UiAction,
    view: &mut CodexMicro,
    window: &mut Window,
    cx: &mut Context<CodexMicro>,
) -> anyhow::Result<()> {
    match command {
        UiAction::Select { control } => view.select(control, window, cx),
        UiAction::Tab { tab } => view.switch_tab(tab, window, cx),
        UiAction::Phase { phase } => {
            anyhow::ensure!(
                (phase == Phase::Step) == view.selected.is_rotation(),
                "Invalid phase for selected control"
            );
            view.switch_phase(phase, cx);
        }
        UiAction::Preset { preset } => {
            view.preset = preset;
            cx.notify();
        }
        UiAction::Scroll { y } => {
            view.action_scroll.set_offset(point(px(0.), px(y)));
            cx.notify();
        }
        UiAction::FocusSearch => view
            .action_search
            .update(cx, |input, cx| input.focus(window, cx)),
        UiAction::FocusInput => view
            .action_input()
            .update(cx, |input, cx| input.focus(window, cx)),
        UiAction::TextSubmit { enabled } => view.set_text_submit(enabled, cx),
        UiAction::ClearShortcut => view.clear_shortcut(window, cx),
        UiAction::ShortcutKeyUp { key } => {
            let keystroke = Keystroke::parse(&key)?;
            view.shortcut_key_up(&KeyUpEvent { keystroke }, window, cx);
        }
        UiAction::ShortcutModifiers { modifiers } => {
            view.shortcut_modifiers(
                &ModifiersChangedEvent {
                    modifiers,
                    ..Default::default()
                },
                window,
                cx,
            );
        }
        UiAction::Save => view.save(window, cx),
        UiAction::Test => view.test(cx),
        UiAction::Remove => view.remove(window, cx),
        UiAction::Reset => view.reset_all(window, cx),
        UiAction::UndoReset => view.undo_reset(window, cx),
        UiAction::OpenProfileDialog => view.open_profile_dialog(window, cx),
        UiAction::CancelProfileDialog => view.cancel_profile_dialog(window, cx),
        UiAction::SubmitProfile => view.submit_profile(window, cx),
        UiAction::ActivateProfile { index } => view.activate_profile(index, window, cx)?,
    }
    Ok(())
}

pub fn type_text(text: &str, window: &mut Window, cx: &mut App) {
    for character in text.chars() {
        window.dispatch_keystroke(
            Keystroke {
                key: character.to_string(),
                key_char: Some(character.to_string()),
                modifiers: Modifiers::default(),
            },
            cx,
        );
    }
}

pub fn poll(
    queue: &mpsc::Receiver<PendingInput>,
    view: &WeakEntity<CodexMicro>,
    cx: &mut AsyncWindowContext,
) {
    for (command, done) in queue.try_iter() {
        let response = match command {
            InputCommand::Ui { action } => view
                .update_in(cx, |view, window, cx| dispatch(action, view, window, cx))
                .and_then(|result| result)
                .map(|_| None),
            InputCommand::Type { text } => cx
                .update(|window, cx| type_text(&text, window, cx))
                .map(|_| None),
            InputCommand::Key { key } => Keystroke::parse(&key)
                .map_err(anyhow::Error::from)
                .and_then(|key| {
                    cx.update(|window, cx| window.dispatch_keystroke(key, cx))
                        .map(|_| None)
                }),
            InputCommand::Inspect => view.update_in(cx, |view, _, cx| {
                Some(Snapshot {
                    query: view.action_search.read(cx).value().to_string(),
                    actions: view
                        .visible_presets(cx)
                        .into_iter()
                        .map(Preset::name)
                        .collect(),
                    dirty: view.is_dirty(cx),
                    phase: view.phase,
                    tab: view.tab.name(),
                    preset: view.preset,
                    scroll_y: f32::from(view.action_scroll.offset().y),
                    input: view.action_input().read(cx).value().to_string(),
                    text_submit: view.text_submit,
                    shortcut_preview: view.shortcut_capture.preview(),
                    shortcut_held: view.shortcut_capture.is_held(),
                    profiles: view
                        .profiles
                        .list()
                        .iter()
                        .map(|profile| profile.name.clone())
                        .collect(),
                    active_profile: view.profiles.active_index(),
                    profile_dialog: view.profile_dialog,
                    profile_name: view.profile_name.read(cx).value().to_string(),
                    profile_error: view.profile_error.read(cx).clone(),
                })
            }),
        };
        let _ = done.send(Reply::from_result(response));
    }
}
