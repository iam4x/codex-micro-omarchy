use crate::wire;
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    io::BufReader,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

pub type Bindings = BTreeMap<String, BTreeMap<String, Value>>;

#[derive(Debug, Deserialize)]
pub struct Snapshot {
    pub query: String,
    pub actions: Vec<String>,
    pub dirty: bool,
    pub phase: String,
    pub tab: String,
    pub preset: String,
    pub scroll_y: f64,
    pub input: String,
    pub text_submit: bool,
    pub shortcut_preview: Option<String>,
    pub shortcut_held: bool,
    pub profiles: Vec<String>,
    pub active_profile: usize,
    pub profile_dialog: bool,
    pub profile_name: String,
    pub profile_error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub error: Option<String>,
    pub state: Option<Snapshot>,
}

#[derive(Deserialize)]
pub struct Client {
    pid: u32,
    address: String,
    class: String,
    title: String,
    monitor: i32,
    at: [i32; 2],
    size: [u32; 2],
}

#[derive(Deserialize)]
struct Monitor {
    id: i32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

pub fn run(program: &str, args: &[&str]) -> Result<String> {
    let mut command = smol::process::Command::new(program);
    command.args(args).kill_on_drop(true);
    let output = smol::block_on(smol::future::or(
        async { command.output().await.context(format!("Run {program}")) },
        async {
            smol::Timer::after(Duration::from_secs(10)).await;
            bail!("{program} exceeded its 10-second timeout")
        },
    ))?;
    ensure!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn wait_until(mut predicate: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if predicate()? {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "Timed out waiting for expected result"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

pub struct NativeApp {
    child: Child,
    temporary: TempDir,
    pub config: PathBuf,
    pub artifacts: PathBuf,
    endpoint: PathBuf,
    accent: String,
    binary: PathBuf,
}

impl NativeApp {
    pub fn start() -> Result<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let artifacts = root.join("artifacts");
        fs::create_dir_all(&artifacts)?;
        let temporary = tempfile::Builder::new()
            .prefix("codex-micro-ui-")
            .tempdir()?;
        let config = temporary.path().join("work-louder/bindings.toml");
        fs::create_dir_all(config.parent().context("Missing configuration directory")?)?;
        fs::write(&config, include_str!("seed.toml"))?;
        let endpoint = temporary.path().join("ui.sock");
        let home = PathBuf::from(env::var_os("HOME").context("HOME is not set")?);
        let binary = env::var_os("CODEX_MICRO_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/bin/codex-micro"));
        let state = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let palette = state.join("omarchy/current/theme/colors.toml");
        let accent = if palette.exists() {
            let colors: toml::Table = toml::from_str(&fs::read_to_string(palette)?)?;
            colors
                .get("accent")
                .and_then(toml::Value::as_str)
                .unwrap_or("#81a1c1")
                .to_owned()
        } else {
            "#81a1c1".into()
        };
        let log = fs::File::create(artifacts.join("native-session.log"))?;
        let child = Command::new(&binary)
            .env("XDG_CONFIG_HOME", temporary.path())
            .env("CODEX_MICRO_CONTROL_SOCKET", &endpoint)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| {
                format!(
                    "Open {}. Install the app or set CODEX_MICRO_BINARY.",
                    binary.display()
                )
            })?;
        let mut app = Self {
            child,
            temporary,
            config,
            artifacts,
            endpoint,
            accent,
            binary,
        };
        wait_until(|| {
            if let Some(status) = app.child.try_wait()? {
                bail!("Test app exited with {status}; see artifacts/native-session.log");
            }
            Ok(app.endpoint.exists() && app.client()?.is_some())
        })?;
        app.prepare_window()?;
        Ok(app)
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn restart(&mut self) -> Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        fs::remove_file(&self.endpoint)?;
        let log = fs::OpenOptions::new()
            .append(true)
            .open(self.artifacts.join("native-session.log"))?;
        self.child = Command::new(&self.binary)
            .env("XDG_CONFIG_HOME", self.temporary.path())
            .env("CODEX_MICRO_CONTROL_SOCKET", &self.endpoint)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        wait_until(|| {
            if let Some(status) = self.child.try_wait()? {
                bail!("Restarted app exited with {status}");
            }
            Ok(self.endpoint.exists() && self.client()?.is_some())
        })?;
        self.prepare_window()
    }

    fn client(&self) -> Result<Option<Client>> {
        let clients: Vec<Client> = serde_json::from_str(&run("hyprctl", &["-j", "clients"])?)?;
        Ok(clients.into_iter().find(|client| client.pid == self.pid()))
    }

    fn window(&self) -> Result<Client> {
        self.client()?.context("Temporary app window disappeared")
    }

    pub fn focus(&self) -> Result<()> {
        let selector = format!("address:{}", self.window()?.address);
        run(
            "hyprctl",
            &[
                "dispatch",
                &format!("hl.dsp.focus({{window={selector:?}}})"),
            ],
        )?;
        let active: Value = serde_json::from_str(&run("hyprctl", &["-j", "activewindow"])?)?;
        ensure!(
            active["pid"] == self.pid(),
            "Temporary window is not focused"
        );
        Ok(())
    }

    fn prepare_window(&self) -> Result<()> {
        let client = self.window()?;
        ensure!(
            client.class == "codex-micro" && client.title == "Codex Micro",
            "Unexpected app identity"
        );
        let monitors: Vec<Monitor> = serde_json::from_str(&run("hyprctl", &["-j", "monitors"])?)?;
        let monitor = monitors
            .into_iter()
            .find(|monitor| monitor.id == client.monitor)
            .context("Monitor not found")?;
        ensure!(
            monitor.scale.is_finite() && monitor.scale > 0.,
            "Invalid monitor scale"
        );
        let x = monitor.x + ((f64::from(monitor.width) / monitor.scale - 1240.) / 2.) as i32;
        let y = monitor.y + ((f64::from(monitor.height) / monitor.scale - 860.) / 2.) as i32;
        let selector = format!("address:{}", client.address);
        for command in [
            format!("hl.dsp.window.float({{window={selector:?},action=\"on\"}})"),
            format!("hl.dsp.window.resize({{window={selector:?},x=1240,y=860,relative=false}})"),
            format!(
                "hl.dsp.window.set_prop({{window={selector:?},prop=\"opacity\",value=\"1.0 1.0\"}})"
            ),
            format!("hl.dsp.window.set_prop({{window={selector:?},prop=\"opaque\",value=\"on\"}})"),
            format!("hl.dsp.window.move({{window={selector:?},x={x},y={y},relative=false}})"),
        ] {
            run("hyprctl", &["dispatch", &command])?;
        }
        thread::sleep(Duration::from_secs(1));
        ensure!(
            self.window()?.size == [1240, 860],
            "Unexpected test viewport"
        );
        Ok(())
    }

    pub fn request(&self, value: Value) -> Result<Reply> {
        let mut socket =
            UnixStream::connect(&self.endpoint).context("Connect to native UI control socket")?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        wire::write(&mut socket, &value)?;
        wire::read(&mut BufReader::new(socket)).with_context(|| format!("Response to {value}"))
    }

    fn send(&self, value: Value) -> Result<Reply> {
        let response = self.request(value.clone())?;
        ensure!(response.ok, "{value} failed: {:?}", response.error);
        thread::sleep(Duration::from_millis(200));
        Ok(response)
    }

    pub fn ui(&self, mut action: Value) -> Result<()> {
        action
            .as_object_mut()
            .context("Expected a UI action object")?
            .insert("op".into(), json!("ui"));
        self.send(action)?;
        Ok(())
    }

    pub fn key(&self, key: &str) -> Result<()> {
        self.send(json!({"op": "key", "key": key}))?;
        Ok(())
    }

    pub fn type_text(&self, text: &str) -> Result<()> {
        self.send(json!({"op": "type", "text": text}))?;
        Ok(())
    }

    pub fn inspect(&self) -> Result<Snapshot> {
        self.request(json!({"op": "inspect"}))?
            .state
            .context("Missing native UI snapshot")
    }

    pub fn bindings(&self) -> Result<Bindings> {
        #[derive(Deserialize)]
        struct Profile {
            #[serde(default)]
            bindings: Bindings,
        }
        #[derive(Deserialize)]
        struct Profiles {
            active: usize,
            profiles: Vec<Profile>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Configuration {
            Multiple(Profiles),
            Legacy(Profile),
        }
        let profile = match toml::from_str(&fs::read_to_string(&self.config)?)? {
            Configuration::Legacy(profile) => profile,
            Configuration::Multiple(mut config) => {
                ensure!(
                    config.active < config.profiles.len(),
                    "Invalid active profile"
                );
                config.profiles.remove(config.active)
            }
        };
        Ok(profile.bindings)
    }

    pub fn binding(&self, control: &str, phase: &str) -> Result<Option<Value>> {
        Ok(self
            .bindings()?
            .get(control)
            .and_then(|phases| phases.get(phase))
            .cloned())
    }

    pub fn screenshot(&self, name: &str) -> Result<()> {
        self.focus()?;
        thread::sleep(Duration::from_millis(150));
        let client = self.window()?;
        let [x, y] = client.at;
        let [w, h] = client.size;
        let path = self.artifacts.join(format!("{name}.png"));
        run(
            "grim",
            &["-g", &format!("{x},{y} {w}x{h}"), &path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn check_field_size(&self, tab: &str) -> Result<()> {
        let path = self.artifacts.join(format!("native-input-{tab}.png"));
        let crop = if tab == "text" {
            "310x210+920+360"
        } else {
            "310x100+920+360"
        };
        let size = run(
            "magick",
            &[
                &path.to_string_lossy(),
                "-crop",
                crop,
                "+repage",
                "-alpha",
                "off",
                "-fuzz",
                "1%",
                "-fill",
                "white",
                "-opaque",
                &self.accent,
                "-fill",
                "black",
                "+opaque",
                "white",
                "-trim",
                "-format",
                "%w %h",
                "info:",
            ],
        )?;
        let dimensions = size
            .split_whitespace()
            .map(str::parse::<u32>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            dimensions.len() == 2,
            "Invalid measured input dimensions: {size}"
        );
        let [width, height] = [dimensions[0], dimensions[1]];
        ensure!(
            width >= 280 && height >= 24,
            "{tab} input collapsed to {width}x{height}"
        );
        ensure!(
            tab != "text" || height >= 150,
            "Text area is too short: {height}"
        );
        Ok(())
    }

    pub fn marker(&self) -> PathBuf {
        self.temporary.path().join("result.txt")
    }
}

impl Drop for NativeApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
