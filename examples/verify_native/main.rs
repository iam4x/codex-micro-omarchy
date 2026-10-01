mod ai;
mod harness;
#[path = "../../src/wire.rs"]
mod wire;

use anyhow::{Context, Result, ensure};
use harness::{NativeApp, wait_until};
use serde_json::json;
use std::{fs, thread, time::Duration};

fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mode = match arguments.as_slice() {
        [] => "all",
        [flag] if flag == "--profiles" => "profiles",
        [flag] if flag == "--ai-only" => "ai",
        [flag] if flag == "--joystick" => "joystick",
        [flag] if flag == "--shortcuts" => "shortcuts",
        [flag] if flag == "--lighting" => "lighting",
        _ => anyhow::bail!(
            "Usage: verify-native [--profiles | --ai-only | --joystick | --shortcuts | --lighting]"
        ),
    };
    let mut app = NativeApp::start()?;
    let result = match mode {
        "profiles" => check_profiles(&mut app),
        "ai" => ai::check(&app),
        "joystick" => check_joystick(&mut app),
        "shortcuts" => check_shortcuts(&app),
        "lighting" => check_lighting(&mut app),
        _ => verify(&mut app),
    };
    if result.is_err() {
        let _ = app.screenshot("native-failure");
    }
    result?;
    drop(app);
    if mode != "profiles"
        && mode != "joystick"
        && mode != "shortcuts"
        && mode != "lighting"
        && std::env::var_os("CODEX_MICRO_VERIFY_REAL_AI").is_none()
    {
        ai::check_window_close()?;
    }
    Ok(())
}

fn verify(app: &mut NativeApp) -> Result<()> {
    ensure!(
        !app.request(json!({"op": "ui", "action": "phase", "phase": "step"}))?
            .ok,
        "Invalid phase accepted"
    );
    ensure!(
        !app.request(json!({"op": "unknown"}))?.ok,
        "Unknown operation accepted"
    );
    ensure!(
        app.request(json!({"op": "inspect"}))?.ok,
        "Control socket stopped responding"
    );
    println!("PASS: invalid UI commands return errors without disabling the control socket");
    check_joystick(app).context("Joystick bindings")?;
    check_profiles(app).context("Multiple profiles")?;
    check_search(app).context("System action search")?;
    check_forms(app).context("Action forms")?;
    check_phases(app).context("Press/release switching")?;
    check_shortcuts(app).context("Shortcut recording")?;
    check_execution(app).context("Service execution")?;
    ai::check(app).context("AI generated bindings")?;
    check_reset(app).context("Reset, undo, and removal")?;
    for preset in [
        "play_pause",
        "play",
        "pause",
        "next_track",
        "previous_track",
    ] {
        app.ui(json!({"action": "tab", "tab": "system"}))?;
        app.ui(json!({"action": "preset", "preset": preset}))?;
        app.ui(json!({"action": "save"}))?;
        ensure!(
            app.binding("AG00", "press")? == Some(json!({"kind": "preset", "preset": preset})),
            "Media assignment failed: {preset}"
        );
    }
    println!("PASS: all five media actions can be assigned");
    app.screenshot("native-media")?;
    println!("PASS: Native GPUI callback and keyboard smoke check");
    Ok(())
}

fn check_joystick(app: &mut NativeApp) -> Result<()> {
    for control in ["JOY_UP", "JOY_RIGHT", "JOY_DOWN", "JOY_LEFT"] {
        app.ui(json!({"action":"select", "control":control}))?;
        ensure!(
            app.inspect()?.phase == "press",
            "Joystick must default to press"
        );
        ensure!(
            !app.request(json!({"op":"ui", "action":"phase", "phase":"step"}))?
                .ok,
            "Joystick accepted a rotation step"
        );
        app.ui(json!({"action":"tab", "tab":"system"}))?;
        for (phase, preset) in [("press", "terminal"), ("release", "browser")] {
            app.ui(json!({"action":"phase", "phase":phase}))?;
            app.ui(json!({"action":"preset", "preset":preset}))?;
            app.ui(json!({"action":"save"}))?;
            ensure!(
                app.binding(control, phase)? == Some(json!({"kind":"preset", "preset":preset})),
                "Joystick binding was not saved: {control} {phase}"
            );
        }
    }
    app.screenshot("native-joystick")?;
    app.restart()?;
    for control in ["JOY_UP", "JOY_RIGHT", "JOY_DOWN", "JOY_LEFT"] {
        app.ui(json!({"action":"select", "control":control}))?;
        for (phase, preset) in [("press", "terminal"), ("release", "browser")] {
            app.ui(json!({"action":"phase", "phase":phase}))?;
            ensure!(
                app.binding(control, phase)? == Some(json!({"kind":"preset", "preset":preset})),
                "Joystick binding did not reload"
            );
            app.ui(json!({"action":"remove"}))?;
            app.ui(json!({"action":"confirm_remove"}))?;
            ensure!(
                app.binding(control, phase)?.is_none(),
                "Joystick binding was not removed"
            );
        }
    }
    app.ui(json!({"action":"select", "control":"AG00"}))?;
    println!("PASS: all joystick directions support press/release, persistence, and removal");
    Ok(())
}

fn check_profiles(app: &mut NativeApp) -> Result<()> {
    let initial = app.inspect()?;
    ensure!(
        initial.profiles == ["Desktop"] && initial.active_profile == 0,
        "Legacy profile was not imported: {initial:?}"
    );
    let original = fs::read_to_string(&app.config)?;
    let bindings = app.bindings()?;
    app.screenshot("native-profile-legacy")?;
    app.ui(json!({"action": "open_profile_dialog"}))?;
    ensure!(app.inspect()?.profile_dialog, "Create dialog did not open");
    app.screenshot("native-profile-dialog")?;
    app.type_text("Cancelled")?;
    app.key("ctrl-s")?;
    ensure!(
        fs::read_to_string(&app.config)? == original,
        "Ctrl+S in the dialog saved a background binding"
    );
    app.ui(json!({"action": "cancel_profile_dialog"}))?;
    ensure!(
        !app.inspect()?.profile_dialog && fs::read_to_string(&app.config)? == original,
        "Cancel changed the saved profiles"
    );
    app.ui(json!({"action": "open_profile_dialog"}))?;
    app.type_text("Escaped")?;
    app.key("escape")?;
    ensure!(
        !app.inspect()?.profile_dialog && fs::read_to_string(&app.config)? == original,
        "Escape did not cancel the dialog"
    );
    app.ui(json!({"action": "open_profile_dialog"}))?;
    app.key("enter")?;
    ensure!(
        app.inspect()?.profile_dialog && fs::read_to_string(&app.config)? == original,
        "Empty name created a profile"
    );
    app.type_text("  Café work  ")?;
    ensure!(
        app.inspect()?.profile_name == "  Café work  ",
        "Profile name input did not receive keyboard text"
    );
    app.key("enter")?;
    let created = app.inspect()?;
    ensure!(
        created.profiles == ["Desktop", "Café work"]
            && created.active_profile == 0
            && !created.profile_dialog,
        "Enter did not create an inactive profile: {created:?}"
    );
    ensure!(
        app.bindings()? == bindings,
        "Creation changed active bindings"
    );
    app.ui(json!({"action": "open_profile_dialog"}))?;
    app.type_text("café WORK")?;
    app.key("enter")?;
    let duplicate = app.inspect()?;
    ensure!(
        duplicate.profile_dialog
            && duplicate.profile_error.is_some()
            && duplicate.profiles.len() == 2,
        "Duplicate name did not stay in the dialog with an error: {duplicate:?}"
    );
    app.screenshot("native-profile-duplicate")?;
    app.ui(json!({"action": "cancel_profile_dialog"}))?;
    app.ui(json!({"action": "activate_profile", "index": 1}))?;
    let switched = app.inspect()?;
    ensure!(
        switched.active_profile == 1 && app.bindings()?.is_empty(),
        "New profile did not activate with empty bindings: {switched:?}"
    );
    let selected_config = fs::read_to_string(&app.config)?;
    app.ui(json!({"action": "activate_profile", "index": 1}))?;
    ensure!(
        app.inspect()?.active_profile == 1 && fs::read_to_string(&app.config)? == selected_config,
        "Active profile could be unchecked"
    );
    ensure!(
        !app.request(json!({"op": "ui", "action": "activate_profile", "index": 99}))?
            .ok,
        "Out-of-range profile accepted"
    );
    app.ui(json!({"action": "tab", "tab": "text"}))?;
    app.type_text("Only in Café work")?;
    app.ui(json!({"action": "save"}))?;
    let work_bindings = app.bindings()?;
    app.ui(json!({"action": "reset"}))?;
    app.ui(json!({"action": "open_profile_dialog"}))?;
    app.type_text("Gaming")?;
    app.key("enter")?;
    app.ui(json!({"action": "undo_reset"}))?;
    ensure!(
        app.bindings()? == work_bindings && app.inspect()?.profiles.len() == 3,
        "Undo reset removed a new profile or failed to restore active bindings"
    );
    app.restart()?;
    let reopened = app.inspect()?;
    ensure!(
        reopened.profiles == ["Desktop", "Café work", "Gaming"]
            && reopened.active_profile == 1
            && reopened.input == "Only in Café work"
            && app.bindings()? == work_bindings,
        "Profiles, activation, or bindings failed to survive restart: {reopened:?}"
    );
    app.screenshot("native-profiles")?;
    app.ui(json!({"action": "reset"}))?;
    app.ui(json!({"action": "activate_profile", "index": 0}))?;
    app.ui(json!({"action": "undo_reset"}))?;
    ensure!(
        app.bindings()? == bindings && app.inspect()?.active_profile == 0,
        "Profile activation leaked reset undo into another profile"
    );
    println!(
        "PASS: native profile dialog handles Cancel, Escape, Enter, Unicode, blank and duplicate names, and isolates Ctrl+S"
    );
    println!(
        "PASS: profiles preserve separate bindings, one active selection, reset isolation, and restart persistence"
    );
    Ok(())
}

fn check_search(app: &NativeApp) -> Result<()> {
    app.screenshot("native-before")?;
    app.ui(json!({"action": "scroll", "y": -500}))?;
    app.screenshot("native-scrolled")?;
    let saved_config = fs::read_to_string(&app.config)?;
    let initial = app.inspect()?;
    ensure!(
        initial.actions.len() == 14,
        "Unexpected action count: {initial:?}"
    );
    app.ui(json!({"action": "focus_search"}))?;
    app.type_text("  VOLUME  ")?;
    let state = app.inspect()?;
    ensure!(
        state.actions == ["Volume up", "Volume down"],
        "Name filtering failed: {state:?}"
    );
    ensure!(
        state.scroll_y == 0. && state.dirty == initial.dirty,
        "Search changed form state: {state:?}"
    );
    app.screenshot("native-search-volume")?;
    app.key("ctrl-a")?;
    app.type_text("5%")?;
    ensure!(
        app.inspect()?.actions == ["Volume up", "Volume down"],
        "Description filtering failed"
    );
    app.key("ctrl-a")?;
    app.type_text("no-such-action")?;
    ensure!(
        app.inspect()?.actions.is_empty(),
        "No-match search returned results"
    );
    app.screenshot("native-search-empty")?;
    app.key("ctrl-a")?;
    app.key("backspace")?;
    let state = app.inspect()?;
    ensure!(
        state.query.is_empty() && state.actions.len() == 14,
        "Search did not clear: {state:?}"
    );
    ensure!(
        state.dirty == initial.dirty && fs::read_to_string(&app.config)? == saved_config,
        "Search edited a binding"
    );
    println!(
        "PASS: keyboard search filters names and descriptions, resets scroll, handles no matches, and clears without editing bindings"
    );
    Ok(())
}

fn check_forms(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action": "select", "control": "AG00"}))?;
    for (tab, value, expected) in [
        (
            "app",
            "notify-send \"Input check\"",
            json!({"kind": "launch", "command": "notify-send \"Input check\""}),
        ),
        (
            "shortcut",
            "Ctrl+Shift+C",
            json!({"kind": "shortcut", "chord": "Ctrl+Shift+C"}),
        ),
        (
            "text",
            "Hello from my Micro\nSecond line\n\nFinal line: café",
            json!({"kind": "text", "text": "Hello from my Micro\nSecond line\n\nFinal line: café", "submit": true}),
        ),
        (
            "command",
            "printf input-check",
            json!({"kind": "command", "command": "printf input-check"}),
        ),
    ] {
        app.ui(json!({"action": "tab", "tab": tab}))?;
        app.screenshot(&format!("native-input-{tab}"))?;
        app.check_field_size(tab)?;
        match tab {
            "shortcut" => {
                app.ui(json!({"action": "shortcut_modifiers", "modifiers": {"control": true, "shift": true}}))?;
                app.key("ctrl-shift-c")?;
                let held = app.inspect()?;
                ensure!(
                    held.input.is_empty()
                        && held.shortcut_preview.as_deref() == Some("Ctrl+Shift+C")
                        && held.shortcut_held,
                    "Unexpected held shortcut: {held:?}"
                );
                app.screenshot("native-shortcut-held")?;
                app.ui(json!({"action": "shortcut_key_up", "key": "ctrl-shift-c"}))?;
                ensure!(
                    app.inspect()?.input.is_empty(),
                    "Shortcut committed before modifier release"
                );
                app.ui(json!({"action": "shortcut_modifiers", "modifiers": {"control": true}}))?;
                let state = app.inspect()?;
                ensure!(
                    state.input.is_empty(),
                    "Shortcut committed with Ctrl still held: {state:?}"
                );
                app.ui(json!({"action": "shortcut_modifiers", "modifiers": {}}))?;
                let released = app.inspect()?;
                ensure!(
                    released.input == value && !released.shortcut_held,
                    "Shortcut did not commit after release: {released:?}"
                );
            }
            "text" => {
                for (index, line) in value.split('\n').enumerate() {
                    if index != 0 {
                        app.key("enter")?;
                    }
                    app.type_text(line)?;
                }
                let state = app.inspect()?;
                ensure!(
                    state.input == value && !state.text_submit,
                    "Textarea input or default submit state is wrong: {state:?}"
                );
                app.ui(json!({"action": "text_submit", "enabled": true}))?;
            }
            _ => app.type_text(value)?,
        }
        app.screenshot(&format!("native-input-{tab}-typed"))?;
        let form = app.inspect()?;
        let before = fs::read_to_string(&app.config)?;
        for phase in ["release", "press", "press"] {
            app.ui(json!({"action": "phase", "phase": phase}))?;
            let state = app.inspect()?;
            ensure!(
                state.phase == phase
                    && state.tab == form.tab
                    && state.input == form.input
                    && state.text_submit == form.text_submit,
                "{tab} form reset when switching to {phase}: {state:?}"
            );
            ensure!(
                state.dirty == form.dirty && fs::read_to_string(&app.config)? == before,
                "Phase switching edited a binding"
            );
        }
        app.ui(json!({"action": "save"}))?;
        ensure!(
            app.binding("AG00", "press")? == Some(expected),
            "{tab} did not save the exact action"
        );
        ensure!(!app.inspect()?.dirty, "Saved {tab} form is still dirty");
        if tab == "text" {
            reload(app)?;
            let state = app.inspect()?;
            ensure!(
                state.input == value && state.text_submit && !state.dirty,
                "Saved text did not reload cleanly: {state:?}"
            );
            app.screenshot("native-text-reloaded")?;
            check_text_delivery(app, value)?;
        }
    }
    println!(
        "PASS: App, Shortcut, Text, and Command fields render at full width and save typed values"
    );
    Ok(())
}

fn reload(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action": "select", "control": "AG01"}))?;
    app.ui(json!({"action": "select", "control": "AG00"}))
}

fn check_text_delivery(app: &NativeApp, value: &str) -> Result<()> {
    for enabled in [false, true] {
        app.ui(json!({"action": "text_submit", "enabled": enabled}))?;
        app.ui(json!({"action": "save"}))?;
        ensure!(
            app.binding("AG00", "press")?
                .context("Missing text binding")?["submit"]
                == enabled,
            "Submit setting did not save"
        );
        app.focus()?;
        app.ui(json!({"action": "focus_input"}))?;
        app.key("ctrl-a")?;
        app.ui(json!({"action": "test"}))?;
        thread::sleep(Duration::from_secs(1));
        let typed = format!("{value}{}", if enabled { "\n" } else { "" });
        wait_until(|| Ok(app.inspect()?.input == typed))
            .context("Text delivery through installed service")?;
        reload(app)?;
        let state = app.inspect()?;
        ensure!(
            state.input == value && state.text_submit == enabled,
            "Text delivery changed the saved snippet: {state:?}"
        );
    }
    println!(
        "PASS: submit toggle saves, reloads, and sends exactly one extra Enter through the real service"
    );
    println!(
        "PASS: textarea accepts Enter, blank lines, and Unicode; saved line breaks reload exactly"
    );
    Ok(())
}

fn check_phases(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action": "select", "control": "AG02"}))?;
    app.ui(json!({"action": "preset", "preset": "play_pause"}))?;
    app.ui(json!({"action": "save"}))?;
    let before = fs::read_to_string(&app.config)?;
    app.ui(json!({"action": "phase", "phase": "release"}))?;
    let state = app.inspect()?;
    ensure!(
        state.tab == "System" && state.preset == "play_pause" && state.dirty,
        "Phase switching lost preset: {state:?}"
    );
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Unsaved phase switch changed configuration"
    );
    app.ui(json!({"action": "save"}))?;
    let expected = json!({"kind": "preset", "preset": "play_pause"});
    ensure!(
        app.binding("AG02", "press")? == Some(expected.clone())
            && app.binding("AG02", "release")? == Some(expected.clone()),
        "Preset did not save to both phases"
    );
    app.ui(json!({"action": "phase", "phase": "press"}))?;
    let state = app.inspect()?;
    ensure!(
        state.preset == "play_pause" && !state.dirty,
        "Matching phase is dirty: {state:?}"
    );
    app.ui(json!({"action": "preset", "preset": "pause"}))?;
    app.ui(json!({"action": "phase", "phase": "release"}))?;
    app.ui(json!({"action": "save"}))?;
    ensure!(
        app.binding("AG02", "press")? == Some(expected)
            && app.binding("AG02", "release")?
                == Some(json!({"kind": "preset", "preset": "pause"})),
        "Save changed another phase"
    );
    app.ui(json!({"action": "phase", "phase": "press"}))?;
    let state = app.inspect()?;
    ensure!(
        state.preset == "pause" && state.dirty,
        "Phase switching discarded edited form: {state:?}"
    );
    println!(
        "PASS: phase switching preserves all forms and edits; Save changes only the selected phase"
    );
    Ok(())
}

fn check_shortcuts(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action": "select", "control": "AG00"}))?;
    let before = fs::read_to_string(&app.config)?;
    app.ui(json!({"action": "tab", "tab": "shortcut"}))?;
    app.ui(json!({"action": "shortcut_modifiers", "modifiers": {"control": true}}))?;
    app.key("ctrl-s")?;
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Ctrl+S ran Save instead of recording"
    );
    app.ui(json!({"action": "save"}))?;
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Pending chord was saved before release"
    );
    app.ui(json!({"action": "shortcut_key_up", "key": "ctrl-s"}))?;
    ensure!(
        app.inspect()?.input.is_empty(),
        "Ctrl+S captured before Ctrl release"
    );
    app.ui(json!({"action": "shortcut_modifiers", "modifiers": {}}))?;
    ensure!(app.inspect()?.input == "Ctrl+S", "Ctrl+S did not record");
    app.key("escape")?;
    ensure!(
        app.inspect()?.input == "Ctrl+S",
        "Esc committed before release"
    );
    app.ui(json!({"action": "shortcut_key_up", "key": "escape"}))?;
    ensure!(
        app.inspect()?.input == "Escape",
        "Esc cancelled instead of recording"
    );
    app.ui(json!({"action": "save"}))?;
    ensure!(
        app.binding("AG00", "press")? == Some(json!({"kind": "shortcut", "chord": "Escape"})),
        "Escape did not save"
    );
    reload(app)?;
    ensure!(app.inspect()?.input == "Escape", "Escape did not reload");
    let before = fs::read_to_string(&app.config)?;
    app.ui(json!({"action": "clear_shortcut"}))?;
    let state = app.inspect()?;
    ensure!(
        state.input.is_empty() && !state.shortcut_held,
        "Clear did not reset field: {state:?}"
    );
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Clear changed a saved binding"
    );
    app.key("ctrl-c")?;
    app.ui(json!({"action": "clear_shortcut"}))?;
    app.ui(json!({"action": "shortcut_key_up", "key": "c"}))?;
    app.ui(json!({"action": "shortcut_modifiers", "modifiers": {}}))?;
    let state = app.inspect()?;
    ensure!(
        state.input.is_empty() && !state.shortcut_held,
        "Clear did not reset pending chord: {state:?}"
    );
    app.key("f5")?;
    ensure!(
        app.inspect()?.input.is_empty(),
        "F5 committed before release"
    );
    app.ui(json!({"action": "shortcut_key_up", "key": "f5"}))?;
    ensure!(app.inspect()?.input == "F5", "Function key did not record");
    app.key("tab")?;
    app.ui(json!({"action": "shortcut_key_up", "key": "tab"}))?;
    ensure!(
        app.inspect()?.input == "Tab",
        "Tab moved focus instead of recording"
    );
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Recording changed a saved binding"
    );
    app.key("ctrl-c")?;
    app.ui(json!({"action": "shortcut_manual", "enabled": true}))?;
    ensure!(
        app.inspect()?.shortcut_manual
            && !app.inspect()?.shortcut_held
            && app.inspect()?.input == "Tab",
        "Switching to typing lost the draft or left a pending capture"
    );
    app.key("ctrl-a")?;
    app.type_text("Super+")?;
    app.ui(json!({"action": "save"}))?;
    ensure!(
        app.inspect()?.message_error && fs::read_to_string(&app.config)? == before,
        "Invalid manual shortcut was saved"
    );
    app.type_text("Left")?;
    let state = app.inspect()?;
    ensure!(
        state.input == "Super+Left" && !state.shortcut_held,
        "Typed shortcut was intercepted by recorder: {state:?}"
    );
    ensure!(
        fs::read_to_string(&app.config)? == before,
        "Typing changed a saved binding"
    );
    app.ui(json!({"action": "save"}))?;
    ensure!(
        app.binding("AG00", "press")? == Some(json!({"kind": "shortcut", "chord": "Super+Left"})),
        "System shortcut did not save"
    );
    app.screenshot("native-shortcut-manual")?;
    reload(app)?;
    ensure!(
        app.inspect()?.input == "Super+Left",
        "System shortcut did not reload"
    );
    app.ui(json!({"action": "shortcut_manual", "enabled": false}))?;
    app.key("f5")?;
    app.ui(json!({"action": "shortcut_key_up", "key": "f5"}))?;
    ensure!(
        !app.inspect()?.shortcut_manual && app.inspect()?.input == "F5",
        "Recording did not resume after manual entry"
    );
    println!(
        "PASS: manual entry saves and reloads Super+Left, cancels pending capture, and returns to recording"
    );
    println!(
        "PASS: shortcut captures only after all releases, suppresses editor shortcuts, handles Escape and function keys, and keeps changes as a draft"
    );
    Ok(())
}

fn check_execution(app: &NativeApp) -> Result<()> {
    app.ui(json!({"action": "tab", "tab": "command"}))?;
    let marker = app.marker();
    let command = format!(
        "printf verified > {}",
        shell_words::quote(&marker.to_string_lossy())
    );
    app.type_text(&command)?;
    app.ui(json!({"action": "save"}))?;
    wait_until(|| {
        Ok(app.binding("AG00", "press")? == Some(json!({"kind": "command", "command": command})))
    })?;
    println!("PASS: editor saved the exact keyboard-entered command to TOML");
    app.screenshot("native-saved")?;
    app.ui(json!({"action": "test"}))?;
    wait_until(|| match fs::read_to_string(&marker) {
        Ok(value) => Ok(value == "verified"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    })?;
    println!("PASS: editor action crossed the service socket and ran the command");
    Ok(())
}

fn check_reset(app: &NativeApp) -> Result<()> {
    let before = app.bindings()?;
    app.ui(json!({"action": "reset"}))?;
    ensure!(app.bindings()?.is_empty(), "Reset did not clear all phases");
    app.screenshot("native-reset")?;
    app.ui(json!({"action": "undo_reset"}))?;
    ensure!(
        app.bindings()? == before,
        "Undo did not restore the exact profile"
    );
    println!("PASS: reset removed all controls and phases; undo restored them exactly");
    app.ui(json!({"action": "confirm_remove"}))?;
    thread::sleep(Duration::from_millis(300));
    app.screenshot("native-remove-dialog")?;
    app.key("escape")?;
    thread::sleep(Duration::from_millis(300));
    ensure!(
        app.bindings()? == before,
        "Cancelling the remove dialog changed bindings"
    );
    app.ui(json!({"action": "confirm_remove"}))?;
    thread::sleep(Duration::from_millis(300));
    app.key("enter")?;
    wait_until(|| Ok(!app.bindings()?.contains_key("AG00")))
        .context("Confirming the remove dialog did not remove the binding")?;
    let bindings = app.bindings()?;
    ensure!(
        !bindings.contains_key("AG00")
            && bindings.contains_key("AG05")
            && bindings.contains_key("ENC_CW"),
        "Remove changed another control"
    );
    println!(
        "PASS: remove asks for confirmation, Escape cancels, Enter clears only the selected binding"
    );
    Ok(())
}

fn check_lighting(app: &mut NativeApp) -> Result<()> {
    let bindings = app.bindings()?;
    let defaults = app.inspect()?.lighting;
    ensure!(
        app.inspect()?.lighting_target.is_none(),
        "Lighting should start without a selected target"
    );
    app.ui(json!({"action":"lighting_page","open":true}))?;
    app.screenshot("native-lighting-unselected")?;
    ensure!(
        !app.request(json!({"op":"ui","action":"lighting_value","light":{"effect":"solid"}}))?
            .ok,
        "Lighting edit accepted without a selected target"
    );
    app.ui(json!({"action":"lighting_page","open":false}))?;
    ensure!(
        defaults["keys"]["effect"] == "solid",
        "Legacy profile must enable default lighting"
    );
    app.ui(json!({"action":"tab","tab":"command"}))?;
    app.type_text("printf keep-lighting-draft")?;
    app.ui(json!({"action":"lighting_page","open":true}))?;
    let effects = [
        "solid",
        "snake",
        "rainbow",
        "breath",
        "gradient",
        "off",
        "shallow_breath",
        "solid",
    ];
    let colors = [
        0xe5edf5, 0x60a5fa, 0x4ade80, 0xfbbf24, 0xc084fc, 0xfb7185, 0xa5c7e8, 0x2dd4bf,
    ];
    for (target, effect) in effects.into_iter().enumerate() {
        app.ui(json!({"action":"lighting_target","target":target}))?;
        app.ui(json!({"action":"lighting_value","light":{"color":colors[target],"effect":effect,"brightness":60,"speed":30}}))?;
        check_saved_lighting(app)?;
        let state = app.inspect()?.lighting;
        let actual = match target {
            0..=5 => &state["agents"][target],
            6 => &state["keys"],
            _ => &state["ambient"],
        };
        ensure!(
            actual == &json!({"color":colors[target],"effect":effect,"brightness":60,"speed":30}),
            "Lighting target {target} did not apply its new value"
        );
    }
    app.ui(json!({"action":"select","control":"AG02"}))?;
    ensure!(
        app.inspect()?.lighting_target == Some(2),
        "Device key did not select Key 03"
    );
    app.ui(json!({"action":"lighting_page","open":false}))?;
    ensure!(
        app.inspect()?.input == "printf keep-lighting-draft",
        "Lighting selection discarded binding draft"
    );
    app.ui(json!({"action":"lighting_page","open":true}))?;
    app.ui(json!({"action":"lighting_target","target":7}))?;
    let lighting = app.inspect()?.lighting;
    ensure!(
        !app.request(json!({"op":"ui","action":"lighting_target","target":8}))?
            .ok,
        "Invalid target accepted"
    );
    let original = fs::read_to_string(&app.config)?;
    app.key("ctrl-s")?;
    ensure!(
        fs::read_to_string(&app.config)? == original && app.bindings()? == bindings,
        "Ctrl+S in lighting changed bindings"
    );
    app.screenshot("native-lighting")?;
    check_slider_release(app, false, "brightness")?;
    app.ui(json!({"action":"lighting_value","light":{"color":0,"effect":"off","brightness":0,"speed":0}}))?;
    check_saved_lighting(app)?;
    app.screenshot("native-lighting-off")?;
    app.ui(json!({"action":"lighting_value","light":{"color":0x2dd4bf,"effect":"breath","brightness":60,"speed":30}}))?;
    check_saved_lighting(app)?;
    app.screenshot("native-lighting-breath")?;
    check_slider_release(app, true, "speed")?;
    let animated = app.inspect()?.lighting;
    app.restart()?;
    ensure!(
        app.inspect()?.lighting == animated,
        "Automatic lighting changes did not reload"
    );
    app.ui(json!({"action":"lighting_page","open":true}))?;
    app.ui(json!({"action":"select","control":"MIC"}))?;
    ensure!(
        app.inspect()?.lighting_target == Some(6),
        "Command key did not select grouped LEDs"
    );
    app.ui(json!({"action":"open_profile_dialog"}))?;
    app.type_text("Lighting test")?;
    app.ui(json!({"action":"submit_profile"}))?;
    app.ui(json!({"action":"activate_profile","index":1}))?;
    ensure!(
        app.inspect()?.lighting == defaults,
        "New profile must enable independent default lighting"
    );
    app.ui(json!({"action":"activate_profile","index":0}))?;
    ensure!(
        app.inspect()?.lighting == animated,
        "Switching profiles lost lighting"
    );
    app.ui(json!({"action":"reset"}))?;
    app.ui(json!({"action":"lighting_target","target":7}))?;
    app.ui(json!({"action":"lighting_value","light":{"color":0xff6600,"effect":"breath","brightness":45,"speed":50}}))?;
    let after_reset = app.inspect()?.lighting;
    app.ui(json!({"action":"undo_reset"}))?;
    ensure!(
        app.inspect()?.lighting == after_reset && app.bindings()? == bindings,
        "Undo reset reverted lighting or lost bindings"
    );
    ensure!(
        lighting["ambient"]["effect"] == "solid",
        "Solid screenshot used wrong effect"
    );
    println!(
        "PASS: lighting saves sliders on release, applies colors and effects immediately, starts without a selection, and preserves profiles and bindings"
    );
    Ok(())
}

fn check_slider_release(app: &NativeApp, speed: bool, field: &str) -> Result<()> {
    let original = fs::read_to_string(&app.config)?;
    let start = app.inspect()?.lighting["ambient"][field].clone();
    for value in [65, 70, 80, 90] {
        app.ui(json!({"action":"lighting_slider","speed":speed,"value":value}))?;
        ensure!(
            fs::read_to_string(&app.config)? == original,
            "Slider saved before release"
        );
    }
    ensure!(
        app.inspect()?.lighting["ambient"][field] != start,
        "Slider drag did not change {field}"
    );
    app.ui(json!({"action":"lighting_slider_release"}))?;
    check_saved_lighting(app)?;
    let saved = fs::read_to_string(&app.config)?;
    ensure!(saved != original, "Slider release did not save");
    app.ui(json!({"action":"lighting_slider_release"}))?;
    ensure!(
        fs::read_to_string(&app.config)? == saved,
        "Repeated release changed settings"
    );
    Ok(())
}

fn check_saved_lighting(app: &NativeApp) -> Result<()> {
    let state = app.inspect()?;
    let saved: toml::Value = toml::from_str(&fs::read_to_string(&app.config)?)?;
    let stored = serde_json::to_value(&saved["profiles"][state.active_profile]["lighting"])?;
    ensure!(
        stored == state.lighting,
        "Lighting change was not saved immediately"
    );
    Ok(())
}
