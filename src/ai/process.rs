use anyhow::{Context, Result, ensure};
use smol::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

const MAX_CAPTURE: usize = 128 * 1024;

#[derive(Default)]
enum State {
    #[default]
    Idle,
    Active(i32),
    Cancelled,
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<Mutex<State>>);

#[derive(Default)]
pub struct GenerationCancellation(CancellationToken);
impl GenerationCancellation {
    pub fn token(&self) -> CancellationToken {
        self.0.clone()
    }
}
impl Drop for GenerationCancellation {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl CancellationToken {
    fn cancel(&self) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let State::Active(pid) = *state {
            kill_group(pid);
        }
        *state = State::Cancelled;
    }
    pub(super) fn check(&self) -> Result<()> {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        ensure!(!matches!(*state, State::Cancelled), "Generation cancelled");
        Ok(())
    }
    fn finish(&self, pid: i32) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if matches!(*state, State::Active(active) if active == pid) {
            // Pipe-holding tool children must stop before the leader is disarmed.
            kill_group(pid);
            *state = State::Idle;
        }
    }
}

fn kill_group(pid: i32) {
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}

pub(super) async fn run_process(
    mut command: Command,
    input: &[u8],
    cancellation: CancellationToken,
) -> Result<std::process::Output> {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    let mut command = smol::process::Command::from(command);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut child, group) = {
        let mut state = cancellation
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ensure!(
            matches!(*state, State::Idle),
            "Generation cancelled or already running"
        );
        let child = command.spawn()?;
        let pid = child.id() as i32;
        *state = State::Active(pid);
        drop(state);
        (child, ProcessGroup { pid, cancellation })
    };
    let mut stdin = child.stdin.take().context("Missing command input")?;
    let stdout = child.stdout.take().context("Missing command output")?;
    let stderr = child.stderr.take().context("Missing command diagnostics")?;
    let write = async {
        stdin.write_all(input).await?;
        drop(stdin);
        Ok::<_, anyhow::Error>(())
    };
    let read = smol::future::try_zip(
        read_bounded(stdout, MAX_CAPTURE),
        read_bounded(stderr, MAX_CAPTURE),
    );
    let status = async {
        let status = child.status().await?;
        group.cancellation.finish(group.pid);
        Ok::<_, anyhow::Error>(status)
    };
    let ((_, (stdout, stderr)), status) =
        smol::future::try_zip(smol::future::try_zip(write, read), status).await?;
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

struct ProcessGroup {
    pid: i32,
    cancellation: CancellationToken,
}
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.cancellation.finish(self.pid);
    }
}

pub(super) async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    maximum: usize,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(
        bytes.len() <= maximum,
        "Codex output exceeded its size limit"
    );
    Ok(bytes)
}
