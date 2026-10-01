use super::*;
use crate::wire;
use std::sync::Weak;

struct Subscription {
    hub: SharedHub,
    client: Weak<SyncSender<Event>>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.hub
            .lock()
            .unwrap()
            .clients
            .retain(|client| Arc::as_ptr(client) != self.client.as_ptr());
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Status,
    Subscribe,
    Test { action: Action },
}

#[derive(Serialize, Deserialize)]
pub struct ServiceStatus {
    #[serde(flatten)]
    pub device: DeviceStatus,
    pub mapped_controls: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Response {
    Status(ServiceStatus),
    Accepted { ok: bool },
    Error { error: String },
}

fn request(request: &Request) -> Result<Response> {
    let mut client =
        UnixStream::connect(socket_path()?).context("The device service is not running")?;
    client.set_read_timeout(Some(Duration::from_secs(3)))?;
    client.set_write_timeout(Some(Duration::from_secs(3)))?;
    wire::write(&mut client, request)?;
    match wire::read(&mut BufReader::new(client))? {
        Response::Error { error } => bail!("{error}"),
        response => Ok(response),
    }
}

pub fn status() -> Result<ServiceStatus> {
    match request(&Request::Status)? {
        Response::Status(status) => Ok(status),
        _ => bail!("Unexpected device status response"),
    }
}

pub fn test(action: &Action) -> Result<()> {
    match request(&Request::Test {
        action: action.clone(),
    })? {
        Response::Accepted { ok: true } => Ok(()),
        _ => bail!("The device service did not accept the action"),
    }
}

fn subscribe() -> Result<BufReader<UnixStream>> {
    let mut client = UnixStream::connect(socket_path()?)?;
    client.set_write_timeout(Some(Duration::from_secs(3)))?;
    wire::write(&mut client, &Request::Subscribe)?;
    Ok(BufReader::new(client))
}

pub fn events() -> mpsc::Receiver<Event> {
    let (sender, receiver) = mpsc::sync_channel(128);
    thread::spawn(move || {
        loop {
            match ensure_running().and_then(|_| subscribe()) {
                Ok(mut stream) => {
                    while let Ok(event) = wire::read(&mut stream) {
                        if sender.send(event).is_err() {
                            return;
                        }
                    }
                }
                Err(error) => {
                    let event = Event::Status(DeviceStatus::Disconnected {
                        message: format!("{error:#}"),
                    });
                    if sender.send(event).is_err() {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
    receiver
}

pub(super) fn serve(mut client: UnixStream, hub: SharedHub) -> Result<()> {
    client.set_read_timeout(Some(Duration::from_secs(3)))?;
    client.set_write_timeout(Some(Duration::from_secs(2)))?;
    let request = wire::read(&mut BufReader::new(client.try_clone()?));
    let response = match request {
        Ok(Request::Status) => {
            let hub = hub.lock().unwrap();
            Response::Status(ServiceStatus {
                device: hub.status.clone(),
                mapped_controls: hub.profile.bindings.len(),
            })
        }
        Ok(Request::Subscribe) => {
            let (sender, receiver) = mpsc::sync_channel(128);
            let sender = Arc::new(sender);
            let _subscription = Subscription {
                hub: hub.clone(),
                client: Arc::downgrade(&sender),
            };
            {
                let mut hub = hub.lock().unwrap();
                sender.send(Event::Status(hub.status.clone()))?;
                hub.clients.push(sender);
            }
            loop {
                let event = match receiver.recv_timeout(Duration::from_secs(2)) {
                    Ok(event) => event,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        Event::Status(hub.lock().unwrap().status.clone())
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                };
                wire::write(&mut client, &event)?;
            }
        }
        Ok(Request::Test { action }) => match execute(&action, hub) {
            Ok(()) => Response::Accepted { ok: true },
            Err(error) => Response::Error {
                error: format!("{error:#}"),
            },
        },
        Err(error) => Response::Error {
            error: format!("{error:#}"),
        },
    };
    wire::write(&mut client, &response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn exchange(bytes: &[u8], hub: SharedHub) -> Response {
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let worker = thread::spawn(move || serve(server, hub).unwrap());
        client.write_all(bytes).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let response = wire::read(&mut BufReader::new(client)).unwrap();
        worker.join().unwrap();
        response
    }

    #[test]
    fn status_is_typed_and_counts_bindings_without_a_device() {
        let hub = Arc::new(Mutex::new(Hub::default()));
        hub.lock().unwrap().profile.assign(
            crate::model::Control::Mic,
            crate::model::Phase::Press,
            Some(Action::Text {
                text: "Hello\nworld".into(),
                submit: false,
            }),
        );
        match exchange(b"{\"method\":\"status\"}\n", hub) {
            Response::Status(status) => assert_eq!(status.mapped_controls, 1),
            _ => panic!("Wrong status response"),
        }
    }

    #[test]
    fn malformed_requests_and_invalid_actions_return_errors() {
        for bytes in [
            b"{broken}\n".as_slice(),
            b"{\"method\":\"unknown\"}\n",
            b"{\"method\":\"test\",\"action\":{\"kind\":\"text\",\"text\":\"\"}}\n",
            b"{\"method\":\"status\"}",
        ] {
            assert!(matches!(
                exchange(bytes, Arc::new(Mutex::new(Hub::default()))),
                Response::Error { .. }
            ));
        }
    }

    #[test]
    fn test_reports_a_failed_spawn_instead_of_accepting_it() {
        let action = Action::Launch {
            command: "/definitely/missing/work-louder-executable".into(),
        };
        let mut bytes = Vec::new();
        wire::write(&mut bytes, &Request::Test { action }).unwrap();
        assert!(matches!(
            exchange(&bytes, Arc::new(Mutex::new(Hub::default()))),
            Response::Error { .. }
        ));
    }

    #[test]
    fn subscriptions_send_the_current_status_then_live_events() {
        let hub = Arc::new(Mutex::new(Hub::default()));
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let server_hub = hub.clone();
        let (done, finished) = mpsc::channel();
        let worker = thread::spawn(move || done.send(serve(server, server_hub)).unwrap());
        wire::write(&mut client, &Request::Subscribe).unwrap();
        let mut reader = BufReader::new(client);
        assert!(matches!(
            wire::read::<Event>(&mut reader).unwrap(),
            Event::Status(DeviceStatus::Connecting)
        ));
        let status = DeviceStatus::Disconnected {
            message: "Waiting for device".into(),
        };
        publish(&hub, Event::Status(status.clone()));
        match wire::read::<Event>(&mut reader).unwrap() {
            Event::Status(received) => assert_eq!(received, status),
            _ => panic!("Wrong subscription event"),
        }
        reader.get_ref().shutdown(std::net::Shutdown::Both).unwrap();
        drop(reader);
        assert!(
            finished
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );
        worker.join().unwrap();
        assert!(hub.lock().unwrap().clients.is_empty());
    }
}
