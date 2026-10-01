//! Exercises text delivery against a native chat input: Enter sends,
//! Shift+Enter adds a line break. No messages leave this temporary window.
#[allow(dead_code)]
#[path = "../src/lighting.rs"]
mod lighting;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/wire.rs"]
mod wire;

use anyhow::{Context as _, Result, ensure};
use gpui::{
    AppContext, Application, Bounds, Context, Entity, KeyBinding, Render, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_component::{
    Root,
    input::{Enter, Input, InputEvent, InputState},
};
use serde_json::{Value, json};
use std::{
    io::BufReader,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
    time::Duration,
};

actions!(chat_probe, [Submit, PastePrimary]);

struct ChatProbe {
    input: Entity<InputState>,
    sent: Vec<String>,
    _subscription: Subscription,
}

impl ChatProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>, outcome: Arc<Mutex<Result<()>>>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).multi_line(true).rows(6));
        let subscription = cx.subscribe(&input, |_, _, _: &InputEvent, cx| cx.notify());
        cx.spawn_in(window, async move |view, cx| {
            let result: Result<()> = async {
                smol::Timer::after(Duration::from_millis(800)).await;
                let output = smol::process::Command::new("hyprctl").args(["-j", "clients"]).output().await?;
                ensure!(output.status.success(), "Cannot find test window");
                let clients: Vec<Value> = serde_json::from_slice(&output.stdout)?;
                let address = clients.iter().find(|client| client["pid"] == std::process::id())
                    .and_then(|client| client["address"].as_str()).context("Test window not found")?;
                let selector = format!("address:{address}");
                let output = smol::process::Command::new("hyprctl")
                    .args(["dispatch", &format!("hl.dsp.focus({{window={selector:?}}})")]).output().await?;
                ensure!(output.status.success(), "Cannot focus test window");
                let clipboard_before = smol::process::Command::new("wl-paste")
                    .arg("--no-newline").output().await?;
                let client = clients.iter().find(|client| client["pid"] == std::process::id()).unwrap();
                let x = client["at"][0].as_i64().context("Missing window x")?
                    + client["size"][0].as_i64().context("Missing window width")? / 2;
                let y = client["at"][1].as_i64().context("Missing window y")? + 100;
                let output = smol::process::Command::new("hyprctl")
                    .args(["dispatch", &format!("hl.dsp.cursor.move({{x={x},y={y},relative=false}})")])
                    .output().await?;
                ensure!(output.status.success(), "Cannot give the test window pointer focus");
                smol::Timer::after(Duration::from_millis(150)).await;
                for snippet in [
                    "Hello\n",
                    "--literal\n\nCafé ☕\n-M shift\n-k Return\n",
                    "\n\n",
                    "Single line: café",
                    "CRLF\r\nNext\rLast\r",
                ] {
                    let normalized = snippet.replace("\r\n", "\n").replace('\r', "\n");
                    for (bulk, submit) in [(false, false), (false, true), (true, false), (true, true)] {
                        view.update_in(cx, |this, window, cx| {
                            this.sent.clear();
                            this.input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        })?;
                        smol::Timer::after(Duration::from_millis(150)).await;
                        let active = smol::process::Command::new("hyprctl").args(["-j", "activewindow"]).output().await?;
                        let active: Value = serde_json::from_slice(&active.stdout)?;
                        ensure!(active["pid"] == std::process::id(), "Test window lost focus; no text sent");
                        let snippet = snippet.to_owned();
                        smol::unblock(move || send_text(&snippet, submit, bulk)).await?;
                        smol::Timer::after(Duration::from_millis(600)).await;
                        let (input, sent) = view.update_in(cx, |this, _, cx| {
                            (this.input.read(cx).value().to_string(), this.sent.clone())
                        })?;
                        if submit {
                            ensure!(input.is_empty() && sent == [normalized.clone()],
                                "Submit enabled must send the complete snippet exactly once: input={input:?}, sent={sent:?}");
                        } else {
                            ensure!(input == normalized && sent.is_empty(),
                                "Submit disabled must preserve the whole draft without sending: input={input:?}, sent={sent:?}");
                        }
                        println!("PASS: bulk={bulk}, submit={submit}, {} line breaks, {} submissions, exact Unicode text",
                            normalized.matches('\n').count(), sent.len());
                    }
                }
                let clipboard_after = smol::process::Command::new("wl-paste")
                    .arg("--no-newline").output().await?;
                ensure!(clipboard_before.status == clipboard_after.status
                    && clipboard_before.stdout == clipboard_after.stdout,
                    "Text delivery changed the normal clipboard");
                println!("PASS: normal clipboard unchanged");
                Ok(())
            }.await;
            *outcome.lock().unwrap() = result;
            let _ = cx.update(|_, cx| cx.quit());
        }).detach();
        Self {
            input,
            sent: Vec::new(),
            _subscription: subscription,
        }
    }

    fn paste_primary(&mut self, _: &PastePrimary, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_primary().and_then(|item| item.text()) {
            self.input
                .update(cx, |input, cx| input.insert(text, window, cx));
            cx.notify();
        }
    }

    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        self.sent.push(self.input.read(cx).value().to_string());
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }
}

impl Render for ChatProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::paste_primary))
            .child("Local chat regression check. Enter submits. Shift+Enter adds a line.")
            .child(Input::new(&self.input).h(px(180.)))
            .child(format!("Submissions: {}", self.sent.len()))
    }
}

fn send_text(text: &str, submit: bool, bulk: bool) -> Result<()> {
    if std::env::var_os("CODEX_MICRO_TEXT_LOCAL").is_some() {
        let argv = model::Action::Text {
            text: text.into(),
            submit,
            bulk,
        }
        .argv()?;
        let status = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .status()?;
        ensure!(status.success(), "Text delivery failed: {status}");
        return Ok(());
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is missing")?;
    let mut stream = UnixStream::connect(std::path::Path::new(&runtime).join("work-louder.sock"))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    wire::write(
        &mut stream,
        &json!({"method": "test", "action": {"kind": "text", "text": text, "submit": submit, "bulk": bulk}}),
    )?;
    let reply: Value = wire::read(&mut BufReader::new(stream))?;
    ensure!(reply["ok"] == true, "Service refused text test: {reply}");
    Ok(())
}

fn main() -> Result<()> {
    let outcome = Arc::new(Mutex::new(Err(anyhow::anyhow!(
        "Test window closed before completion"
    ))));
    let result = outcome.clone();
    let startup_result = outcome.clone();
    Application::new().run(move |cx| {
        gpui_component::init(cx);
        cx.bind_keys([
            KeyBinding::new("enter", Submit, Some("Input")),
            KeyBinding::new("shift-insert", PastePrimary, Some("Input")),
            KeyBinding::new("shift-enter", Enter { secondary: false }, Some("Input")),
        ]);
        cx.on_window_closed(|cx| cx.quit()).detach();
        let bounds = Bounds::centered(None, size(px(720.), px(380.)), cx);
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("codex-micro-text-probe".into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("Codex Micro text regression".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| ChatProbe::new(window, cx, outcome));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            Ok(_) => cx.activate(true),
            Err(error) => {
                *startup_result.lock().unwrap() = Err(error);
                cx.quit();
            }
        }
    });
    let mut result = result.lock().unwrap();
    std::mem::replace(&mut *result, Ok(()))
}
