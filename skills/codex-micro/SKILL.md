---
name: codex-micro
description: Create shell automations and button or dial bindings for the Codex Micro Omarchy application. Use for Codex Micro AI action generation or requests to configure its saved bindings.
---

# Codex Micro

Turn a desired desktop action into a repeatable shell script. Discover the
installed application and window identifiers before choosing a launch command.
Use the supplied desktop context when generation runs inside the app.

## In-app generation

Return one JSON object with exactly `summary` and `script`, both strings.
`summary` briefly explains the behavior. `script` contains the complete POSIX
shell script, without Markdown fences. The app checks it with `sh -n`, displays
it for review, and saves it only when the user selects Assign action or
Save changes.

Generate for the selected control and event. Do not edit live bindings, launch
applications, install packages, or run the generated automation during this
generation task. Read-only inspection of installed commands, application
launchers, and desktop configuration is useful when the context is insufficient.

The device service executes the saved script with `sh -c`. It runs with the
user's normal permissions and does not call Codex. Do not rely on the temporary
generation directory, its files, or the app being open. Prefer commands already
installed on the machine. Quote variables and paths; use `$HOME` and XDG paths
instead of embedding an account name. Use POSIX syntax rather than Bash-only
arrays or `[[ ... ]]`.

## Desktop actions

For focus-or-launch tasks, match an existing window by its observed class,
focus its address, and launch only when no matching window exists. An app's
display name, desktop file ID, executable, and window class can differ. Use
the installed `.desktop` entry or supplied `Exec` command to resolve the
launcher. Remove desktop field codes such as `%U` when calling an executable
directly. Prefer `gtk-launch <desktop-file-id>` when available.

This Omarchy installation uses Hyprland's Lua dispatch API. Focus a window with:

```sh
hyprctl dispatch "hl.dsp.focus({window=\"address:$address\"})"
```

Read `hyprctl -j clients` for current addresses and classes. Do not bake a current
window address or PID into the saved script; discover it again on each press.
Inspect the installed Hyprland helpers for unfamiliar dispatch operations.
Use `omarchy <group> <action>` for Omarchy commands, and inspect `omarchy commands`
or the command's help when selecting an operation.

## Saved bindings

The app keeps `~/.config/work-louder/bindings.toml`, or the same relative path
under `$XDG_CONFIG_HOME`. The device service reloads valid changes automatically.
The file stores a `profiles` array and a zero-based `active` profile index.
Each profile has its own name and bindings. Preserve other profiles and edit
only the selected profile's control/event when configuring bindings externally.
The editor owns the profiles while open; an external edit can be overwritten by
the editor's next Save. In-app generation returns a candidate instead of editing
this file.

Buttons 01 through 06 use `AG00` through `AG05`; buttons 07 through 10 use
`ACT06` through `ACT09`. Button 11 is `MIC`, and button 12 is `ACT12`.
The dial uses `ENC_CLK`, `ENC_CW`, and `ENC_CC`. Joystick up, right, down,
and left use `JOY_UP`, `JOY_RIGHT`, `JOY_DOWN`, and `JOY_LEFT`.

Buttons, joystick directions, and dial press accept `press` or `release`. Dial rotation accepts only
`step`. Each control/event stores one action; preserve unrelated events when
editing a profile. The AI action stores its prompt, explanation, and runnable
script together:

```toml
active = 0

[[profiles]]
name = "Desktop"

[profiles.bindings.AG00.press]
kind = "ai"
prompt = "Show a desktop notification"
summary = "Show a notification from the Micro"
script = "notify-send 'Codex Micro' 'Hello'"
```

Legacy files with top-level `name` and `bindings` still load. The app writes
the multi-profile format on the next save.

The other action kinds are `preset`, `launch`, `shortcut`, `text`, and `command`.
Text snippets send embedded line breaks as Shift+Enter. Their `submit` flag
adds one plain Enter at the end and defaults to false.

Use `codex-micro --check-config` to validate an externally edited profile.
Use the repository README and Rust action model for the current preset list
and detailed configuration examples.
