use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Control {
    AG00,
    AG01,
    AG02,
    AG03,
    AG04,
    AG05,
    ACT06,
    ACT07,
    ACT08,
    ACT09,
    #[serde(rename = "MIC")]
    Mic,
    ACT12,
    #[serde(rename = "ENC_CLK")]
    DialPress,
    #[serde(rename = "ENC_CW")]
    DialClockwise,
    #[serde(rename = "ENC_CC")]
    DialCounterclockwise,
    #[serde(rename = "JOY_UP")]
    JoystickUp,
    #[serde(rename = "JOY_RIGHT")]
    JoystickRight,
    #[serde(rename = "JOY_DOWN")]
    JoystickDown,
    #[serde(rename = "JOY_LEFT")]
    JoystickLeft,
}

impl Control {
    pub const ALL: [Self; 19] = [
        Self::AG00,
        Self::AG01,
        Self::AG02,
        Self::AG03,
        Self::AG04,
        Self::AG05,
        Self::ACT06,
        Self::ACT07,
        Self::ACT08,
        Self::ACT09,
        Self::Mic,
        Self::ACT12,
        Self::DialPress,
        Self::DialClockwise,
        Self::DialCounterclockwise,
        Self::JoystickUp,
        Self::JoystickRight,
        Self::JoystickDown,
        Self::JoystickLeft,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::AG00 => "AG00",
            Self::AG01 => "AG01",
            Self::AG02 => "AG02",
            Self::AG03 => "AG03",
            Self::AG04 => "AG04",
            Self::AG05 => "AG05",
            Self::ACT06 => "ACT06",
            Self::ACT07 => "ACT07",
            Self::ACT08 => "ACT08",
            Self::ACT09 => "ACT09",
            Self::Mic => "MIC",
            Self::ACT12 => "ACT12",
            Self::DialPress => "ENC_CLK",
            Self::DialClockwise => "ENC_CW",
            Self::DialCounterclockwise => "ENC_CC",
            Self::JoystickUp => "JOY_UP",
            Self::JoystickRight => "JOY_RIGHT",
            Self::JoystickDown => "JOY_DOWN",
            Self::JoystickLeft => "JOY_LEFT",
        }
    }
    pub fn from_id(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|control| control.id() == value)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::AG00 => "Key 1",
            Self::AG01 => "Key 2",
            Self::AG02 => "Key 3",
            Self::AG03 => "Key 4",
            Self::AG04 => "Key 5",
            Self::AG05 => "Key 6",
            Self::ACT06 => "Key 7",
            Self::ACT07 => "Key 8",
            Self::ACT08 => "Key 9",
            Self::ACT09 => "Key 10",
            Self::Mic => "Key 11",
            Self::ACT12 => "Key 12",
            Self::DialPress => "Dial press",
            Self::DialClockwise => "Dial clockwise",
            Self::DialCounterclockwise => "Dial counterclockwise",
            Self::JoystickUp => "Joystick up",
            Self::JoystickRight => "Joystick right",
            Self::JoystickDown => "Joystick down",
            Self::JoystickLeft => "Joystick left",
        }
    }
    pub fn short_name(self) -> &'static str {
        match self {
            Self::AG00 => "01",
            Self::AG01 => "02",
            Self::AG02 => "03",
            Self::AG03 => "04",
            Self::AG04 => "05",
            Self::AG05 => "06",
            Self::ACT06 => "07",
            Self::ACT07 => "08",
            Self::ACT08 => "09",
            Self::ACT09 => "10",
            Self::Mic => "11",
            Self::ACT12 => "12",
            Self::DialPress => "Press",
            Self::DialClockwise => "Clockwise",
            Self::DialCounterclockwise => "Counterclockwise",
            Self::JoystickUp => "Up",
            Self::JoystickRight => "Right",
            Self::JoystickDown => "Down",
            Self::JoystickLeft => "Left",
        }
    }
    pub fn is_rotation(self) -> bool {
        matches!(self, Self::DialClockwise | Self::DialCounterclockwise)
    }
    pub fn primary_phase(self) -> Phase {
        if self.is_rotation() {
            Phase::Step
        } else {
            Phase::Press
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Press,
    Release,
    Step,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Press => "On press",
            Self::Release => "On release",
            Self::Step => "On turn",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Preset {
        preset: Preset,
    },
    Launch {
        command: String,
    },
    Shortcut {
        chord: String,
    },
    Text {
        text: String,
        #[serde(default)]
        bulk: bool,
        #[serde(default)]
        submit: bool,
    },
    Command {
        command: String,
    },
    Ai {
        prompt: String,
        summary: String,
        script: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Terminal,
    Browser,
    Files,
    VolumeUp,
    VolumeDown,
    Mute,
    PlayPause,
    Play,
    Pause,
    NextTrack,
    PreviousTrack,
    WorkspaceNext,
    WorkspacePrevious,
    Screenshot,
}
impl Preset {
    pub const ALL: [Self; 14] = [
        Self::Terminal,
        Self::Browser,
        Self::Files,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::Mute,
        Self::PlayPause,
        Self::Play,
        Self::Pause,
        Self::NextTrack,
        Self::PreviousTrack,
        Self::WorkspaceNext,
        Self::WorkspacePrevious,
        Self::Screenshot,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Terminal => "Open terminal",
            Self::Browser => "Web browser",
            Self::Files => "File manager",
            Self::VolumeUp => "Volume up",
            Self::VolumeDown => "Volume down",
            Self::Mute => "Mute audio",
            Self::PlayPause => "Play / pause",
            Self::Play => "Play",
            Self::Pause => "Pause",
            Self::NextTrack => "Next track",
            Self::PreviousTrack => "Previous track",
            Self::WorkspaceNext => "Next workspace",
            Self::WorkspacePrevious => "Previous workspace",
            Self::Screenshot => "Take screenshot",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Browser => "globe",
            Self::Files => "folder",
            Self::VolumeUp | Self::VolumeDown | Self::Mute => "volume",
            Self::PlayPause | Self::Play => "play",
            Self::Pause => "pause",
            Self::NextTrack => "skip-forward",
            Self::PreviousTrack => "skip-back",
            Self::WorkspaceNext | Self::WorkspacePrevious => "layers",
            Self::Screenshot => "camera",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Terminal => "Your default Omarchy terminal",
            Self::Browser => "Your default web browser",
            Self::Files => "Browse files with Nautilus",
            Self::VolumeUp => "Increase by 5% with volume overlay",
            Self::VolumeDown => "Decrease by 5% with volume overlay",
            Self::Mute => "Toggle mute with on-screen feedback",
            Self::PlayPause => "Toggle the active media player",
            Self::Play => "Resume playback",
            Self::Pause => "Pause playback",
            Self::NextTrack => "Skip forward to the next track",
            Self::PreviousTrack => "Go back to the previous track",
            Self::WorkspaceNext => "Move one workspace forward",
            Self::WorkspacePrevious => "Move one workspace back",
            Self::Screenshot => "Open the region capture tool",
        }
    }

    fn argv(self) -> &'static [&'static str] {
        match self {
            Self::Terminal => &["omarchy", "launch", "terminal"],
            Self::Browser => &["omarchy", "launch", "browser"],
            Self::Files => &["omarchy", "launch", "nautilus"],
            Self::VolumeUp => &["omarchy", "audio", "output", "volume", "raise"],
            Self::VolumeDown => &["omarchy", "audio", "output", "volume", "lower"],
            Self::Mute => &["omarchy", "audio", "output", "volume", "mute-toggle"],
            Self::PlayPause => &["omarchy-shell", "media", "playPause"],
            Self::Play => &["omarchy-shell", "media", "play"],
            Self::Pause => &["omarchy-shell", "media", "pause"],
            Self::NextTrack => &["omarchy-shell", "media", "next"],
            Self::PreviousTrack => &["omarchy-shell", "media", "previous"],
            Self::WorkspaceNext => &["hyprctl", "dispatch", "hl.dsp.focus({workspace=\"e+1\"})"],
            Self::WorkspacePrevious => {
                &["hyprctl", "dispatch", "hl.dsp.focus({workspace=\"e-1\"})"]
            }
            Self::Screenshot => &["omarchy", "capture", "screenshot", "region"],
        }
    }
}

impl Action {
    pub fn title(&self) -> String {
        match self {
            Self::Preset { preset } => preset.name().into(),
            Self::Launch { command } => command.clone(),
            Self::Shortcut { chord } => chord.clone(),
            Self::Text { bulk: true, .. } => "Paste text".into(),
            Self::Text { .. } => "Type text".into(),
            Self::Command { .. } => "Run command".into(),
            Self::Ai { summary, .. } => summary.clone(),
        }
    }
    pub fn argv(&self) -> Result<Vec<String>> {
        let argv: Vec<String> = match self {
            Self::Preset { preset } => preset
                .argv()
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            Self::Launch { command } => {
                let argv = shell_words::split(command)
                    .context("Check the quotes in your application command")?;
                ensure!(
                    !argv.is_empty() && !argv[0].is_empty(),
                    "Enter an application command"
                );
                argv
            }
            Self::Shortcut { chord } => {
                let (modifiers, key) = parse_shortcut(chord)?;
                let mods = serde_json::to_string(&modifiers)?;
                let key = serde_json::to_string(&key)?;
                vec![
                    "hyprctl".into(),
                    "eval".into(),
                    format!(
                        "hl.dispatch(hl.dsp.send_key_state({{mods={mods},key={key},state=\"down\"}})); hl.timer(function() hl.dispatch(hl.dsp.send_key_state({{mods={mods},key={key},state=\"up\"}})) end, {{timeout=50,type=\"oneshot\"}})"
                    ),
                ]
            }
            Self::Text { text, submit, bulk } => {
                ensure!(!text.is_empty(), "Enter some text");
                ensure!(
                    !text.contains('\0'),
                    "Actions cannot contain null characters"
                );
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                if *bulk {
                    return Ok(vec![
                        "sh".into(),
                        "-c".into(),
                        include_str!("text-paste.sh").into(),
                        "text-paste".into(),
                        normalized,
                        submit.to_string(),
                    ]);
                }
                let mut argv = vec!["wtype".into()];
                for (index, line) in normalized.split('\n').enumerate() {
                    if index != 0 {
                        argv.extend(
                            ["-M", "shift", "-k", "Return", "-m", "shift"].map(String::from),
                        );
                    }
                    // wtype parses leading hyphens as options. Send them as keys
                    // so modifier commands can still follow this text segment.
                    let mut remaining = line;
                    while let Some(rest) = remaining.strip_prefix('-') {
                        argv.extend(["-k", "minus"].map(String::from));
                        remaining = rest;
                    }
                    if !remaining.is_empty() {
                        argv.push(remaining.into());
                    }
                }
                if *submit {
                    argv.extend(["-s", "500", "-k", "Return"].map(String::from));
                }
                argv
            }
            Self::Command { command } => {
                ensure!(!command.trim().is_empty(), "Enter a command");
                vec!["sh".into(), "-c".into(), command.clone()]
            }
            Self::Ai {
                prompt,
                summary,
                script,
            } => {
                ensure!(
                    !prompt.trim().is_empty(),
                    "Describe what this action should do"
                );
                ensure!(prompt.len() <= 16 * 1024, "AI prompts must be under 16 KiB");
                ensure!(
                    !summary.trim().is_empty() && !script.trim().is_empty(),
                    "Generate a script first"
                );
                ensure!(summary.len() <= 1024, "AI summary must be under 1 KiB");
                ensure!(script.len() <= 64 * 1024, "AI scripts must be under 64 KiB");
                ensure!(
                    !prompt.contains('\0') && !summary.contains('\0'),
                    "AI actions cannot contain null characters"
                );
                vec!["sh".into(), "-c".into(), script.clone()]
            }
        };
        ensure!(
            argv.iter().all(|value| !value.contains('\0')),
            "Actions cannot contain null characters"
        );
        Ok(argv)
    }
}

pub fn parse_shortcut(chord: &str) -> Result<(String, String)> {
    let mut parts: Vec<&str> = chord.split('+').map(str::trim).collect();
    let key = parts.pop().unwrap_or_default();
    ensure!(
        !key.is_empty() && key.chars().all(|ch| ch.is_alphanumeric() || ch == '_'),
        "Use a shortcut such as Ctrl+Shift+C or F5"
    );
    let mut modifiers = Vec::new();
    for modifier in parts {
        let value = match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "CTRL",
            "alt" => "ALT",
            "shift" => "SHIFT",
            "super" | "meta" | "win" => "SUPER",
            _ => bail!("Unknown modifier: {modifier}"),
        };
        ensure!(
            !modifiers.contains(&value),
            "Repeated shortcut modifier: {modifier}"
        );
        modifiers.push(value);
    }
    ensure!(
        !matches!(
            key.to_ascii_lowercase().as_str(),
            "ctrl" | "control" | "alt" | "shift" | "super" | "meta" | "win" | "fn"
        ),
        "Add a key after the modifier"
    );
    Ok((modifiers.join(" + "), key.into()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub lighting: crate::lighting::Lighting,
    #[serde(default)]
    pub bindings: BTreeMap<Control, BTreeMap<Phase, Action>>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Desktop".into(),
            lighting: crate::lighting::Lighting::default(),
            bindings: BTreeMap::new(),
        }
    }
}
impl Profile {
    fn validate(&self) -> Result<()> {
        self.lighting.validate()?;
        for (control, bindings) in &self.bindings {
            for (phase, action) in bindings {
                ensure!(
                    (*phase == Phase::Step) == control.is_rotation(),
                    "Invalid event for {}",
                    control.name()
                );
                action.argv()?;
            }
        }
        Ok(())
    }
    pub fn action(&self, control: Control, phase: Phase) -> Option<&Action> {
        self.bindings.get(&control)?.get(&phase)
    }
    pub fn assign(&mut self, control: Control, phase: Phase, action: Option<Action>) {
        if let Some(action) = action {
            self.bindings
                .entry(control)
                .or_default()
                .insert(phase, action);
        } else if let Some(bindings) = self.bindings.get_mut(&control) {
            bindings.remove(&phase);
            if bindings.is_empty() {
                self.bindings.remove(&control);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profiles {
    profiles: Vec<Profile>,
    active: usize,
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            profiles: vec![Profile::default()],
            active: 0,
        }
    }
}

impl Profiles {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(source) => Self::parse(&source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).context("Could not read saved profiles"),
        }
    }

    pub fn parse(source: &str) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum SavedProfiles {
            Collection(Profiles),
            Legacy(Profile),
        }
        let saved: SavedProfiles =
            toml::from_str(source).context("Could not read saved profiles")?;
        let profiles = match saved {
            SavedProfiles::Collection(profiles) => profiles,
            SavedProfiles::Legacy(profile) => Self {
                profiles: vec![profile],
                active: 0,
            },
        };
        profiles.validate()?;
        Ok(profiles)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path.parent().context("Invalid configuration path")?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }

    pub fn list(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &Profile {
        &self.profiles[self.active]
    }

    pub fn active_mut(&mut self) -> &mut Profile {
        &mut self.profiles[self.active]
    }

    pub fn add(&mut self, name: &str) -> Result<usize> {
        let name = name.trim();
        ensure!(!name.is_empty(), "Enter a profile name");
        ensure!(
            !self
                .profiles
                .iter()
                .any(|profile| { profile.name.trim().to_lowercase() == name.to_lowercase() }),
            "A profile with this name already exists"
        );
        let index = self.profiles.len();
        self.profiles.push(Profile {
            name: name.into(),
            ..Profile::default()
        });
        Ok(index)
    }

    pub fn activate(&mut self, index: usize) -> Result<()> {
        ensure!(index < self.profiles.len(), "Profile does not exist");
        self.active = index;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.profiles.is_empty(),
            "At least one profile is required"
        );
        ensure!(
            self.active < self.profiles.len(),
            "Active profile does not exist"
        );
        for profile in &self.profiles {
            profile.validate()?;
        }
        Ok(())
    }
}

pub fn config_path() -> PathBuf {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        });
    root.join("work-louder/bindings.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ai_action_round_trip_executes_only_the_saved_script() {
        let action = Action::Ai {
            prompt: "Focus an application\nLaunch if closed".into(),
            summary: "Focus application".into(),
            script: "printf '%s' '$HOME'\ntrue".into(),
        };
        assert_eq!(action.title(), "Focus application");
        assert_eq!(
            action.argv().unwrap(),
            ["sh", "-c", "printf '%s' '$HOME'\ntrue"]
        );
        assert_eq!(
            toml::from_str::<Action>(&toml::to_string(&action).unwrap()).unwrap(),
            action
        );
        let mut profile = Profile::default();
        profile.assign(Control::AG02, Phase::Release, Some(action.clone()));
        let decoded = Profiles::parse(&toml::to_string(&profile).unwrap()).unwrap();
        assert_eq!(
            decoded.active().action(Control::AG02, Phase::Release),
            Some(&action)
        );
        let mut profiles = decoded;
        let other = profiles.add("Other").unwrap();
        profiles.activate(other).unwrap();
        let decoded = Profiles::parse(&toml::to_string(&profiles).unwrap()).unwrap();
        assert!(decoded.active().bindings.is_empty());
        assert_eq!(
            decoded.list()[0].action(Control::AG02, Phase::Release),
            Some(&action)
        );
    }
    #[test]
    fn ai_actions_reject_incomplete_oversized_and_null_fields() {
        for (prompt, summary, script) in [
            ("".into(), "Title".into(), "true".into()),
            ("Intent".into(), "".into(), "true".into()),
            ("Intent".into(), "Title".into(), "".into()),
            ("x".repeat(16 * 1024 + 1), "Title".into(), "true".into()),
            ("Intent".into(), "x".repeat(1025), "true".into()),
            ("Intent".into(), "Title".into(), "x".repeat(64 * 1024 + 1)),
            ("Intent\0".into(), "Title".into(), "true".into()),
            ("Intent".into(), "Title\0".into(), "true".into()),
            ("Intent".into(), "Title".into(), "true\0".into()),
        ] {
            assert!(
                Action::Ai {
                    prompt,
                    summary,
                    script
                }
                .argv()
                .is_err()
            );
        }
    }
    #[test]
    fn snippet_line_breaks_do_not_send_unmodified_enter() {
        let action = Action::Text {
            bulk: false,
            text: "Hello\n\nCafé\n".into(),
            submit: false,
        };
        assert_eq!(
            action.argv().unwrap(),
            [
                "wtype", "Hello", "-M", "shift", "-k", "Return", "-m", "shift", "-M", "shift",
                "-k", "Return", "-m", "shift", "Café", "-M", "shift", "-k", "Return", "-m",
                "shift",
            ]
        );
    }
    #[test]
    fn text_submit_defaults_off_and_preserves_the_saved_snippet() {
        let original = "--literal text\n\nCafé\n";
        let legacy: Action = toml::from_str("kind = 'text'\ntext = 'hello'").unwrap();
        assert_eq!(
            legacy,
            Action::Text {
                bulk: false,
                text: "hello".into(),
                submit: false
            }
        );
        for submit in [false, true] {
            let action = Action::Text {
                bulk: false,
                text: original.into(),
                submit,
            };
            let decoded: Action = toml::from_str(&toml::to_string(&action).unwrap()).unwrap();
            assert_eq!(decoded, action);
            let mut expected = vec![
                "wtype",
                "-k",
                "minus",
                "-k",
                "minus",
                "literal text",
                "-M",
                "shift",
                "-k",
                "Return",
                "-m",
                "shift",
                "-M",
                "shift",
                "-k",
                "Return",
                "-m",
                "shift",
                "Café",
                "-M",
                "shift",
                "-k",
                "Return",
                "-m",
                "shift",
            ];
            if submit {
                expected.extend(["-s", "500", "-k", "Return"]);
            }
            assert_eq!(action.argv().unwrap(), expected);
        }
    }
    #[test]
    fn bulk_text_preserves_literal_content_and_saved_mode() {
        for submit in [false, true] {
            let action = Action::Text {
                text: "--literal '$HOME' `echo danger`\r\nCafé ☕\r\n".into(),
                bulk: true,
                submit,
            };
            let decoded: Action = toml::from_str(&toml::to_string(&action).unwrap()).unwrap();
            assert_eq!(decoded, action);
            let argv = action.argv().unwrap();
            assert_eq!(argv[0..2], ["sh", "-c"]);
            assert_eq!(argv[2], include_str!("text-paste.sh"));
            assert_eq!(argv[4], "--literal '$HOME' `echo danger`\nCafé ☕\n");
            assert_eq!(argv[5], submit.to_string());
        }
        assert!(
            Action::Text {
                text: "bad\0text".into(),
                bulk: true,
                submit: true
            }
            .argv()
            .is_err()
        );
    }
    #[test]
    fn bulk_text_does_not_submit_when_copy_or_paste_fails() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        for (name, script) in [
            ("wl-copy", "#!/bin/sh\nexit \"$COPY_EXIT\"\n"),
            (
                "wtype",
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$TEST_ROOT/keys\"\nexit \"$PASTE_EXIT\"\n",
            ),
        ] {
            fs::write(root.join(name), script).unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let argv = Action::Text {
            text: "Hello".into(),
            bulk: true,
            submit: true,
        }
        .argv()
        .unwrap();
        for (copy_exit, paste_exit) in [(1, 0), (0, 1)] {
            let status = std::process::Command::new(&argv[0])
                .args(&argv[1..])
                .env(
                    "PATH",
                    format!("{}:{}", root.display(), std::env::var("PATH").unwrap()),
                )
                .env("XDG_RUNTIME_DIR", root)
                .env("TEST_ROOT", root)
                .env("COPY_EXIT", copy_exit.to_string())
                .env("PASTE_EXIT", paste_exit.to_string())
                .status()
                .unwrap();
            assert!(!status.success());
            let keys = fs::read_to_string(root.join("keys")).unwrap_or_default();
            assert!(!keys.contains("Return"), "Submit ran after failed delivery");
        }
    }
    #[test]
    fn executable_arguments_reject_null_characters() {
        for action in [
            Action::Text {
                bulk: false,
                text: "text\0".into(),
                submit: false,
            },
            Action::Launch {
                command: "printf 'text\0'".into(),
            },
            Action::Command {
                command: "echo text\0".into(),
            },
        ] {
            assert!(action.argv().is_err());
        }
        assert_eq!(
            Action::Text {
                bulk: false,
                text: "Line 1\n\nCafé".into(),
                submit: false,
            }
            .argv()
            .unwrap(),
            [
                "wtype", "Line 1", "-M", "shift", "-k", "Return", "-m", "shift", "-M", "shift",
                "-k", "Return", "-m", "shift", "Café",
            ]
        );
    }

    #[test]
    fn text_delivery_handles_crlf_and_literal_wtype_options() {
        assert_eq!(
            Action::Text {
                bulk: false,
                text: "-M shift\r\n--\r-k Return".into(),
                submit: true
            }
            .argv()
            .unwrap(),
            [
                "wtype", "-k", "minus", "M shift", "-M", "shift", "-k", "Return", "-m", "shift",
                "-k", "minus", "-k", "minus", "-M", "shift", "-k", "Return", "-m", "shift", "-k",
                "minus", "k Return", "-s", "500", "-k", "Return",
            ]
        );
        for submit in [false, true] {
            let mut expected = vec!["wtype", "Café ☕"];
            if submit {
                expected.extend(["-s", "500", "-k", "Return"]);
            }
            assert_eq!(
                Action::Text {
                    bulk: false,
                    text: "Café ☕".into(),
                    submit
                }
                .argv()
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn shortcuts_validate_at_boundary() {
        assert_eq!(
            parse_shortcut("Ctrl+Shift+C").unwrap(),
            ("CTRL + SHIFT".into(), "C".into())
        );
        for value in [
            "",
            "Ctrl+",
            "Ctrl",
            "Win",
            "Fn",
            "Wrong+C",
            "Ctrl+Ctrl+C",
            "A, activewindow",
        ] {
            assert!(parse_shortcut(value).is_err(), "{value}");
        }
    }
    #[test]
    fn profile_round_trip_preserves_release_and_shell_text() {
        let mut profile = Profile::default();
        profile.assign(
            Control::Mic,
            Phase::Release,
            Some(Action::Command {
                command: "printf '%s' '$HOME'".into(),
            }),
        );
        let decoded: Profile = toml::from_str(&toml::to_string(&profile).unwrap()).unwrap();
        assert_eq!(profile.bindings, decoded.bindings);
    }
    #[test]
    fn profiles_migrate_legacy_bindings_without_rewriting_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut legacy = Profile {
            name: "Édition".into(),
            ..Profile::default()
        };
        legacy.assign(
            Control::Mic,
            Phase::Release,
            Some(Action::Text {
                bulk: false,
                text: "Café\n\n".into(),
                submit: true,
            }),
        );
        let source = toml::to_string_pretty(&legacy).unwrap();
        fs::write(&path, &source).unwrap();
        let mut profiles = Profiles::load(&path).unwrap();
        assert_eq!(profiles.list().len(), 1);
        assert_eq!(profiles.active_index(), 0);
        assert_eq!(profiles.active().name, legacy.name);
        assert_eq!(profiles.active().bindings, legacy.bindings);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        profiles.add("Work").unwrap();
        profiles.save(&path).unwrap();
        let loaded = Profiles::load(&path).unwrap();
        assert_eq!(loaded.list().len(), 2);
        assert_eq!(loaded.active().bindings, legacy.bindings);
        assert!(loaded.list()[1].bindings.is_empty());
    }

    #[test]
    fn adding_profiles_trims_names_and_preserves_the_active_profile() {
        let mut profiles = Profiles::default();
        profiles.active_mut().assign(
            Control::AG00,
            Phase::Press,
            Some(Action::Preset {
                preset: Preset::Terminal,
            }),
        );
        assert_eq!(profiles.add("  Café  ").unwrap(), 1);
        assert_eq!(profiles.active_index(), 0);
        assert_eq!(profiles.list()[1].name, "Café");
        assert!(profiles.list()[1].bindings.is_empty());
        for name in ["", "  ", "desktop", "  CAFÉ  "] {
            assert!(profiles.add(name).is_err(), "{name}");
        }
        assert_eq!(profiles.list().len(), 2);
        assert!(profiles.activate(99).is_err());
        assert_eq!(profiles.active_index(), 0);
    }

    #[test]
    fn profiles_persist_active_selection_and_keep_bindings_isolated() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut profiles = Profiles::load(&path).unwrap();
        let first = Action::Preset {
            preset: Preset::Terminal,
        };
        let second = Action::Preset {
            preset: Preset::Browser,
        };
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Press, Some(first.clone()));
        let other = profiles.add("Work").unwrap();
        profiles.activate(other).unwrap();
        assert!(profiles.active().bindings.is_empty());
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Press, Some(second.clone()));
        profiles.save(&path).unwrap();
        let mut loaded = Profiles::load(&path).unwrap();
        assert_eq!(loaded.active_index(), 1);
        assert_eq!(
            loaded.active().action(Control::AG00, Phase::Press),
            Some(&second)
        );
        loaded.activate(0).unwrap();
        assert_eq!(
            loaded.active().action(Control::AG00, Phase::Press),
            Some(&first)
        );
        assert_eq!(
            loaded.list()[1].action(Control::AG00, Phase::Press),
            Some(&second)
        );
        assert_eq!(loaded.active_index(), 0);
    }

    #[test]
    fn profiles_reject_invalid_collections_and_inactive_bindings() {
        for source in [
            "profiles = []\nactive = 0",
            "active = 1\n[[profiles]]\nname = 'Desktop'",
            "active = 0\n[[profiles]]\nname = 'Desktop'\nunknown = 1",
            "active = 0\nname = 'Desktop'",
            "active = 0\n[[profiles]]\nname = 'Desktop'\n[[profiles]]\nname = 'Work'\n[profiles.bindings.AG00.step]\nkind = 'preset'\npreset = 'terminal'",
        ] {
            assert!(Profiles::parse(source).is_err(), "{source}");
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut profiles = Profiles::default();
        profiles.add("Work").unwrap();
        profiles.save(&path).unwrap();
        let original = fs::read(&path).unwrap();
        profiles.activate(1).unwrap();
        profiles.active_mut().assign(
            Control::AG00,
            Phase::Step,
            Some(Action::Preset {
                preset: Preset::Terminal,
            }),
        );
        profiles.activate(0).unwrap();
        assert!(profiles.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn launch_keeps_quoted_arguments_together() {
        assert_eq!(
            Action::Launch {
                command: "notify-send 'Hello world'".into()
            }
            .argv()
            .unwrap(),
            ["notify-send", "Hello world"]
        );
    }

    #[test]
    fn invalid_save_does_not_replace_an_existing_profile() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        Profiles::default().save(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let mut profile = Profiles::default();
        profile.active_mut().assign(
            Control::AG00,
            Phase::Step,
            Some(Action::Text {
                bulk: false,
                text: "wrong phase".into(),
                submit: false,
            }),
        );
        assert!(profile.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(Profiles::parse("name = 'Desktop'\nignored = 'data'").is_err());
    }

    #[test]
    fn simultaneous_saves_produce_complete_profiles_and_leave_no_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        std::thread::scope(|scope| {
            for index in 0..8 {
                let path = &path;
                scope.spawn(move || {
                    let mut profile = Profiles::default();
                    profile.active_mut().name = format!("Writer {index}");
                    profile.active_mut().assign(
                        Control::Mic,
                        Phase::Release,
                        Some(Action::Text {
                            bulk: false,
                            text: "Line 1\n\nCafé".repeat(100),
                            submit: false,
                        }),
                    );
                    for _ in 0..4 {
                        profile.save(path).unwrap();
                        assert_eq!(
                            Profiles::load(path).unwrap().active().bindings,
                            profile.active().bindings
                        );
                    }
                });
            }
        });
        assert!(
            Profiles::load(&path)
                .unwrap()
                .active()
                .name
                .starts_with("Writer ")
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
