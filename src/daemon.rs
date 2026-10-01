mod config;
mod rpc;
pub use rpc::{events, status, test};

use crate::{
    model::{Action, Profile, config_path},
    protocol::{self, Decoder, Input, InputDecoder},
};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use hidapi::{BusType, HidApi, HidDevice};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::BufReader,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeviceStatus {
    #[default]
    Connecting,
    Disconnected {
        message: String,
    },
    Connected {
        transport: String,
        battery: u8,
        charging: bool,
        firmware: String,
        layer: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum Event {
    Status(DeviceStatus),
    Input(Input),
    Action {
        title: String,
        message: String,
        success: bool,
    },
    ConfigError(String),
    LightingApplied,
}

#[derive(Default)]
struct Hub {
    status: DeviceStatus,
    profile: Profile,
    clients: Vec<Arc<SyncSender<Event>>>,
}
impl Hub {
    fn emit(&mut self, event: Event) {
        if let Event::Status(status) = &event {
            self.status = status.clone();
        }
        self.clients
            .retain(|client| client.try_send(event.clone()).is_ok());
    }
}
type SharedHub = Arc<Mutex<Hub>>;
fn publish(hub: &SharedHub, event: Event) {
    if let Ok(mut hub) = hub.lock() {
        hub.emit(event);
    }
}

fn socket_path() -> Result<PathBuf> {
    let directory = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?)
            .join(".cache/work-louder"),
    };
    Ok(directory.join("work-louder.sock"))
}

pub fn ensure_running() -> Result<()> {
    if status().is_ok() {
        return Ok(());
    }
    let started_service = Command::new("systemctl")
        .args(["--user", "start", "codex-micro.service"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !started_service {
        Command::new(std::env::current_exe()?)
            .arg("--daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    }
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(50));
        if status().is_ok() {
            return Ok(());
        }
    }
    bail!("Could not start the device service")
}

fn execute(action: &Action, hub: SharedHub) -> Result<()> {
    let title = action.title();
    let argv = action.argv()?;
    let mut child = smol::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Cannot launch {}", argv[0]))?;
    publish(
        &hub,
        Event::Action {
            title: title.clone(),
            message: "Started".into(),
            success: true,
        },
    );
    smol::spawn(async move {
        let (success, message) = match child.status().await {
            Ok(code) if code.success() => (true, "Completed".into()),
            Ok(code) => (false, format!("Command exited with {code}")),
            Err(error) => (false, error.to_string()),
        };
        publish(
            &hub,
            Event::Action {
                title,
                message,
                success,
            },
        );
    })
    .detach();
    Ok(())
}

fn open_device() -> Result<(HidDevice, String)> {
    let api = HidApi::new()?;
    let info = api
        .device_list()
        .find(|info| info.vendor_id() == protocol::VID && info.product_id() == protocol::PID)
        .context("Connect your Codex Micro with USB or Bluetooth")?;
    let transport = if matches!(info.bus_type(), BusType::Bluetooth) {
        "Bluetooth"
    } else {
        "USB"
    };
    let device = info
        .open_device(&api)
        .context("Cannot access the Micro. Check the device access rule")?;
    Ok((device, transport.into()))
}
fn send(device: &HidDevice, value: &Value) -> Result<()> {
    for report in protocol::reports(value)? {
        let count = device.write(&report)?;
        anyhow::ensure!(count == 64, "Incomplete HID write");
    }
    Ok(())
}

fn device_loop(hub: SharedHub) {
    loop {
        let connection = (|| -> Result<()> {
            let (device, transport) = open_device()?;
            let mut decoder = Decoder::default();
            let mut inputs = InputDecoder::default();
            let mut report = [0; 64];
            let mut next_status = Instant::now();
            let mut request_id = 0;
            let mut status_id = 0;
            let mut layer = None;
            let mut lighting_sync = crate::lighting::Sync::default();
            let mut lighting_ids = Vec::new();
            let mut lighting_failed = false;
            let mut last_response = Instant::now();
            loop {
                if Instant::now() >= next_status {
                    request_id += 1;
                    status_id = request_id;
                    send(&device, &protocol::status_request(status_id))?;
                    next_status = Instant::now() + Duration::from_secs(3);
                }
                let lighting = hub.lock().unwrap().profile.lighting.clone();
                let lighting_requests = if lighting_ids.is_empty() {
                    lighting_sync.update(layer, &lighting, &mut request_id)
                } else {
                    Vec::new()
                };
                for request in lighting_requests {
                    if lighting_ids.is_empty() {
                        lighting_failed = false;
                    }
                    lighting_ids.push((request["id"].as_u64().unwrap(), Instant::now()));
                    send(&device, &request)?;
                }
                anyhow::ensure!(
                    lighting_ids
                        .iter()
                        .all(|(_, sent)| sent.elapsed() < Duration::from_secs(10)),
                    "Device did not acknowledge lighting"
                );
                let count = device.read_timeout(&mut report, 80)?;
                if count > 0 {
                    for message in decoder.feed(&report[..count])? {
                        let response_id = message.get("id").and_then(Value::as_u64);
                        if response_id.is_some_and(|id| {
                            lighting_ids.iter().any(|(pending, _)| *pending == id)
                        }) {
                            lighting_ids.retain(|(id, _)| Some(*id) != response_id);
                            if let Some(error) = message.get("error") {
                                lighting_failed = true;
                                publish(
                                    &hub,
                                    Event::ConfigError(format!(
                                        "Device rejected lighting: {error}"
                                    )),
                                );
                            } else if lighting_ids.is_empty() && !lighting_failed {
                                publish(&hub, Event::LightingApplied);
                            }
                        } else if response_id == Some(status_id) {
                            if let Some(result) = message.get("result") {
                                #[derive(Deserialize)]
                                struct Status {
                                    battery: u8,
                                    is_charging: bool,
                                    version: String,
                                    layer_index: u64,
                                }
                                let result: Status = serde_json::from_value(result.clone())
                                    .context("Unexpected device status")?;
                                layer = Some(result.layer_index);
                                let status = DeviceStatus::Connected {
                                    transport: transport.clone(),
                                    battery: result.battery,
                                    charging: result.is_charging,
                                    firmware: result.version,
                                    layer: result.layer_index,
                                };
                                last_response = Instant::now();
                                if hub.lock().unwrap().status != status {
                                    publish(&hub, Event::Status(status));
                                }
                            }
                        } else {
                            for input in inputs.notification(&message) {
                                if layer == Some(1) {
                                    lighting_sync.input(&input, Instant::now());
                                }
                                publish(&hub, Event::Input(input.clone()));
                                let action = hub
                                    .lock()
                                    .unwrap()
                                    .profile
                                    .action(input.control, input.phase)
                                    .cloned();
                                if let Some(action) = action
                                    && let Err(error) = execute(&action, hub.clone())
                                {
                                    publish(
                                        &hub,
                                        Event::Action {
                                            title: action.title(),
                                            message: error.to_string(),
                                            success: false,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
                anyhow::ensure!(
                    last_response.elapsed() < Duration::from_secs(10),
                    "Device is not responding. Check the connection"
                );
            }
        })();
        if let Err(error) = connection {
            let status = DeviceStatus::Disconnected {
                message: format!("{error:#}"),
            };
            if hub.lock().unwrap().status != status {
                publish(&hub, Event::Status(status));
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}

pub fn run() -> Result<()> {
    let path = socket_path()?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path.parent().context("Invalid runtime directory")?)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("lock"))?;
    if let Err(error) = lock.try_lock_exclusive() {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            return Ok(());
        }
        return Err(error).context("Cannot lock the device service");
    }
    if path.exists() {
        fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let hub = Arc::new(Mutex::new(Hub::default()));
    config::start(hub.clone());
    let worker_hub = hub.clone();
    thread::spawn(move || device_loop(worker_hub));
    for client in listener.incoming() {
        let hub = hub.clone();
        match client {
            Ok(client) => {
                thread::spawn(move || {
                    if let Err(error) = rpc::serve(client, hub) {
                        eprintln!("Client: {error}");
                    }
                });
            }
            Err(error) => eprintln!("Socket: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_processes_report_start_and_nonzero_exit() {
        let (sender, receiver) = mpsc::sync_channel(8);
        let hub = Arc::new(Mutex::new(Hub {
            clients: vec![Arc::new(sender)],
            ..Hub::default()
        }));
        execute(
            &Action::Command {
                command: "exit 7".into(),
            },
            hub,
        )
        .unwrap();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            Event::Action { success: true, .. }
        ));
        match receiver.recv_timeout(Duration::from_secs(3)).unwrap() {
            Event::Action {
                success, message, ..
            } => {
                assert!(!success);
                assert!(message.contains('7'), "{message}");
            }
            _ => panic!("Wrong execution event"),
        }
    }

    #[test]
    fn full_subscribers_disconnect_so_reconnect_can_receive_the_current_status() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut hub = Hub {
            clients: vec![Arc::new(sender)],
            ..Hub::default()
        };
        hub.emit(Event::Status(DeviceStatus::Connecting));
        let latest = DeviceStatus::Disconnected {
            message: "Reconnecting".into(),
        };
        hub.emit(Event::Status(latest.clone()));
        assert!(hub.clients.is_empty());
        assert_eq!(hub.status, latest);
        receiver.recv().unwrap();
        assert!(receiver.recv().is_err());
    }
}
