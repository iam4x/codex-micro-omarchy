use crate::harness::{NativeApp, wait_until};
use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{
    fs, thread,
    time::{Duration, Instant},
};

pub fn check(app: &NativeApp) -> Result<()> {
    let real = std::env::var_os("CODEX_MICRO_VERIFY_REAL_AI").is_some();
    app.ui(json!({"action":"select", "control":"AG04"}))?;
    app.ui(json!({"action":"tab", "tab":"ai"}))?;
    let before = fs::read_to_string(&app.config)?;
    for action in ["save", "test"] {
        app.ui(json!({"action":action}))?;
        ensure!(
            app.inspect()?.message_error,
            "Incomplete AI draft accepted by {action}"
        );
        ensure!(
            fs::read_to_string(&app.config)? == before,
            "Incomplete AI draft modified bindings"
        );
    }
    app.ui(json!({"action":"focus_input"}))?;
    let prompt = format!(
        "Write exactly ai-verified (no newline) into the file {} when this button is pressed.\nUse POSIX sh and do not write or execute it during generation.",
        shell_words::quote(&app.ai_marker().to_string_lossy())
    );
    type_prompt(app, &prompt)?;
    app.screenshot("native-ai-prompt")?;
    app.ui(json!({"action":"generate_ai"}))?;
    app.ui(json!({"action":"phase", "phase":"release"}))?;
    wait_ready(
        app,
        if real {
            Duration::from_secs(310)
        } else {
            Duration::from_secs(10)
        },
    )?;
    let ready = app.inspect()?;
    ensure!(
        ready.input == prompt && !ready.ai_script.is_empty() && !ready.ai_summary.is_empty(),
        "Generated AI draft is incomplete: {ready:?}"
    );
    ensure!(
        ready.phase == "release",
        "Generation reset the selected phase"
    );
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Generation edited live bindings"
    );
    ensure!(
        !app.ai_marker().exists(),
        "Generation executed the requested action"
    );
    app.screenshot("native-ai-ready")?;
    app.ui(json!({"action":"save"}))?;
    let expected =
        json!({"kind":"ai", "prompt":prompt, "summary":ready.ai_summary, "script":ready.ai_script});
    ensure!(
        app.binding("AG04", "release")? == Some(expected.clone()),
        "AI save did not preserve prompt and generated result"
    );
    ensure!(!app.inspect()?.dirty, "Saved AI draft remains dirty");
    app.ui(json!({"action":"phase", "phase":"press"}))?;
    ensure!(
        app.inspect()?.ai_script == ready.ai_script,
        "Phase switch discarded the generated script"
    );
    app.ui(json!({"action":"save"}))?;
    app.ui(json!({"action":"select", "control":"AG03"}))?;
    app.ui(json!({"action":"select", "control":"AG04"}))?;
    let reloaded = app.inspect()?;
    ensure!(
        reloaded.tab == "AI"
            && reloaded.ai_status == "ready"
            && reloaded.input == prompt
            && reloaded.ai_script == ready.ai_script
            && !reloaded.dirty,
        "Saved AI binding did not reload: {reloaded:?}"
    );
    let calls = app.ai_calls()?;
    app.ui(json!({"action":"test"}))?;
    wait_until(|| match fs::read_to_string(app.ai_marker()) {
        Ok(value) => Ok(value == "ai-verified"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    })
    .context("AI script execution through installed service")?;
    ensure!(
        app.ai_calls()? == calls,
        "Executing AI binding invoked Codex again"
    );
    println!(
        "PASS: AI textarea generates a reviewed script, preserves phases, saves and reloads intent, and runs the saved script through the service"
    );
    if !real {
        check_stale_results(app)?;
        check_profile_isolation(app)?;
    }
    app.ui(json!({"action":"select", "control":"AG00"}))?;
    Ok(())
}

fn check_profile_isolation(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action":"select", "control":"AG04"}))?;
    let original = app.inspect()?;
    app.ui(json!({"action":"open_profile_dialog"}))?;
    app.type_text("AI isolation")?;
    app.ui(json!({"action":"submit_profile"}))?;
    let other = app.inspect()?.profiles.len() - 1;
    replace_prompt(app, "AI_FIXTURE_DELAY switch profile")?;
    app.ui(json!({"action":"generate_ai"}))?;
    app.ui(json!({"action":"activate_profile", "index":other}))?;
    let saved = fs::read_to_string(&app.config)?;
    thread::sleep(Duration::from_millis(2200));
    let switched = app.inspect()?;
    ensure!(
        switched.active_profile == other
            && switched.ai_status == "idle"
            && switched.ai_script.is_empty()
            && app.bindings()?.is_empty()
            && fs::read_to_string(&app.config)? == saved,
        "A cancelled generation changed the other profile: {switched:?}"
    );
    app.ui(json!({"action":"activate_profile", "index":original.active_profile}))?;
    let restored = app.inspect()?;
    ensure!(
        restored.tab == "AI"
            && restored.ai_status == "ready"
            && restored.ai_script == original.ai_script
            && restored.input == original.input
            && !restored.dirty,
        "The original profile lost its saved AI action: {restored:?}"
    );
    println!(
        "PASS: profile switching cancels AI generation and preserves the original profile's saved action"
    );
    Ok(())
}

fn type_prompt(app: &NativeApp, prompt: &str) -> Result<()> {
    for (index, line) in prompt.split('\n').enumerate() {
        if index > 0 {
            app.key("enter")?;
        }
        app.type_text(line)?;
    }
    Ok(())
}
fn replace_prompt(app: &NativeApp, prompt: &str) -> Result<()> {
    app.ui(json!({"action":"focus_input"}))?;
    app.key("ctrl-a")?;
    type_prompt(app, prompt)
}
fn wait_ready(app: &NativeApp, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let state = app.inspect()?;
        if state.ai_status == "ready" {
            return Ok(());
        }
        ensure!(
            state.ai_status != "failed",
            "AI generation failed: {}",
            state.message
        );
        ensure!(
            Instant::now() < deadline,
            "AI generation timed out: {state:?}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}
fn check_stale_results(app: &NativeApp) -> Result<()> {
    let saved = fs::read_to_string(&app.config)?;
    replace_prompt(app, "AI_FIXTURE_DELAY create a candidate")?;
    ensure!(
        app.inspect()?.ai_status == "idle",
        "Prompt edit retained an old generated script"
    );
    app.ui(json!({"action":"generate_ai"}))?;
    ensure!(
        app.inspect()?.ai_status == "running",
        "Delayed generation did not start"
    );
    for action in ["save", "test"] {
        app.ui(json!({"action":action}))?;
        ensure!(
            app.inspect()?.message_error,
            "Running generation accepted by {action}"
        );
        ensure!(
            fs::read_to_string(&app.config)? == saved,
            "Pending generation modified bindings"
        );
    }
    replace_prompt(app, "New prompt after cancellation")?;
    thread::sleep(Duration::from_millis(2200));
    let state = app.inspect()?;
    ensure!(
        state.ai_status == "idle"
            && state.ai_script.is_empty()
            && state.input == "New prompt after cancellation",
        "Stale result overwrote edited prompt: {state:?}"
    );
    replace_prompt(app, "AI_FIXTURE_DELAY switch control")?;
    app.ui(json!({"action":"generate_ai"}))?;
    app.ui(json!({"action":"select", "control":"AG03"}))?;
    thread::sleep(Duration::from_millis(2200));
    ensure!(
        app.inspect()?.ai_status == "idle",
        "Stale result survived control selection"
    );
    app.ui(json!({"action":"tab", "tab":"ai"}))?;
    type_prompt(app, "AI_FIXTURE_DELAY cancel")?;
    app.ui(json!({"action":"generate_ai"}))?;
    app.ui(json!({"action":"cancel_ai"}))?;
    ensure!(
        app.inspect()?.ai_status == "idle",
        "Cancel left a running generation"
    );
    app.ui(json!({"action":"generate_ai"}))?;
    app.ui(json!({"action":"tab", "tab":"command"}))?;
    thread::sleep(Duration::from_millis(2200));
    ensure!(
        app.inspect()?.ai_status == "idle",
        "Stale result survived category switching"
    );
    app.ui(json!({"action":"tab", "tab":"ai"}))?;
    type_prompt(app, "AI_FIXTURE_FAIL")?;
    app.ui(json!({"action":"generate_ai"}))?;
    wait_until(|| Ok(app.inspect()?.ai_status == "failed"))?;
    let state = app.inspect()?;
    ensure!(
        state.ai_script.is_empty()
            && state.message_error
            && state.message.contains("fixture generation failed"),
        "Codex error did not remain visible: {state:?}"
    );
    ensure!(
        fs::read_to_string(&app.config)? == saved,
        "Cancelled or failed generation changed bindings"
    );
    println!(
        "PASS: AI rejects unfinished actions, cancels stale prompts, controls and tabs, and surfaces generation errors without editing saved bindings"
    );
    Ok(())
}

pub fn check_window_close() -> Result<()> {
    let mut app = NativeApp::start()?;
    app.ui(json!({"action":"tab", "tab":"ai"}))?;
    type_prompt(&app, "AI_FIXTURE_WINDOW_CLOSE track cleanup")?;
    app.ui(json!({"action":"generate_ai"}))?;
    wait_until(|| Ok(app.ai_pids()?.is_some()))?;
    let (leader, child) = app.ai_pids()?.context("Missing generator fixture PIDs")?;
    app.close()?;
    let result = wait_until(|| Ok(!process_alive(leader) && !process_alive(child)));
    if result.is_err() {
        unsafe {
            libc::kill(-leader, libc::SIGKILL);
        }
    }
    result.context("Closing GPUI window must stop Codex and its tool children")?;
    println!("PASS: closing the native window cancels Codex and its process group");
    Ok(())
}
fn process_alive(pid: i32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|source| {
            source
                .rsplit_once(") ")
                .map(|(_, fields)| !fields.starts_with('Z'))
        })
        .unwrap_or(false)
}
