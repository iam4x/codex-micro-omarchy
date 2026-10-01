use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

struct Fixture {
    root: tempfile::TempDir,
    program: PathBuf,
}
impl Fixture {
    fn new(response: &str, behavior: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let program = root.path().join("codex");
        fs::write(root.path().join("response.json"), response).unwrap();
        let path = shell_words::quote(&root.path().to_string_lossy()).into_owned();
        let source = format!(
            "#!/bin/sh\nset -eu\nroot={path}\nprintf '%s\\n' \"$@\" > \"$root/args\"\npwd > \"$root/cwd\"\nprintf '%s' \"$$\" > \"$root/pid\"\ncat > \"$root/prompt\"\nwhile [ \"$#\" -gt 0 ]; do\n case \"$1\" in -o) result=$2; shift;; --output-schema) schema=$2; shift;; esac\n shift\ndone\ncp \"$schema\" \"$root/schema\"\ncp SKILL.md \"$root/skill\"\n{behavior}\n"
        );
        fs::write(&program, source).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, program }
    }
    fn generate(&self, timeout: Duration) -> Result<GeneratedScript> {
        smol::block_on(generate_with(
            self.program.as_os_str(),
            request(),
            "Desktop fixture".into(),
            timeout,
        ))
    }
    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.root.path().join(name)).unwrap()
    }
}
fn request() -> GenerationRequest {
    GenerationRequest {
        prompt: "Write a harmless marker\nKeep it repeatable".into(),
        control: Control::AG01,
        phase: Phase::Release,
    }
}
async fn generate_with(
    program: &std::ffi::OsStr,
    request: GenerationRequest,
    desktop: String,
    timeout: Duration,
) -> Result<GeneratedScript> {
    super::generate_with(
        program,
        request,
        desktop,
        timeout,
        CancellationToken::default(),
    )
    .await
}
fn good_response() -> &'static str {
    r#"{"summary":"Write marker","script":"printf verified"}"#
}
fn alive(pid: i32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|source| {
            source
                .rsplit_once(") ")
                .map(|(_, fields)| !fields.starts_with('Z'))
        })
        .unwrap_or(false)
}
async fn wait_for_pid(path: &Path) -> i32 {
    for _ in 0..100 {
        if let Ok(source) = fs::read_to_string(path) {
            return source.parse().unwrap();
        }
        smol::Timer::after(Duration::from_millis(10)).await;
    }
    panic!("Fixture child did not start")
}
async fn assert_stopped(pid: i32) {
    for _ in 0..100 {
        if !alive(pid) {
            return;
        }
        smol::Timer::after(Duration::from_millis(10)).await;
    }
    panic!("Generator descendant {pid} survived cancellation")
}

#[test]
fn generation_contract_uses_isolation_skill_schema_and_stdin() {
    let fixture = Fixture::new(good_response(), "cp \"$root/response.json\" \"$result\"");
    let result = fixture.generate(Duration::from_secs(3)).unwrap();
    assert_eq!(result.summary, "Write marker");
    assert_eq!(result.script, "printf verified");
    let args = fixture.read("args");
    for argument in [
        "exec",
        "--ignore-user-config",
        "--ignore-rules",
        MODEL,
        "model_reasoning_effort=\"medium\"",
        "service_tier=\"priority\"",
        "--sandbox",
        "read-only",
        "approval_policy=\"never\"",
        "--ephemeral",
        "--skip-git-repo-check",
        "--output-schema",
    ] {
        assert!(
            args.lines().any(|line| line == argument),
            "Missing argument {argument}: {args}"
        );
    }
    assert!(args.ends_with("-\n"));
    assert!(!args.contains(&request().prompt));
    let cwd = PathBuf::from(fixture.read("cwd").trim());
    assert_ne!(cwd, PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    assert!(!cwd.exists(), "Generation workspace did not clean up");
    assert!(fixture.read("prompt").contains(&request().prompt));
    assert!(fixture.read("prompt").contains("AG01 / On release"));
    assert!(fixture.read("prompt").contains("Desktop fixture"));
    assert_eq!(fixture.read("skill"), SKILL);
    assert_eq!(fixture.read("schema"), SCHEMA);
}

#[test]
fn generation_rejects_failed_cli_and_missing_or_malformed_output() {
    for (response, behavior, message) in [
        (
            good_response(),
            "echo 'model unavailable' >&2; exit 7",
            "model unavailable",
        ),
        (good_response(), "true", "did not return"),
        (
            "not json",
            "cp \"$root/response.json\" \"$result\"",
            "invalid script response",
        ),
        (
            r#"{"summary":"x","script":"true","other":1}"#,
            "cp \"$root/response.json\" \"$result\"",
            "invalid script response",
        ),
        (
            r#"{"summary":"x","script":""}"#,
            "cp \"$root/response.json\" \"$result\"",
            "Generate a script first",
        ),
        (
            r#"{"summary":"x","script":"if then"}"#,
            "cp \"$root/response.json\" \"$result\"",
            "invalid shell syntax",
        ),
    ] {
        let fixture = Fixture::new(response, behavior);
        let error = fixture.generate(Duration::from_secs(3)).unwrap_err();
        assert!(format!("{error:#}").contains(message), "{error:#}");
    }
}

#[test]
fn syntax_validation_never_executes_the_script() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("should-not-exist");
    let script = format!(
        "printf 'oops' > {}",
        shell_words::quote(&marker.to_string_lossy())
    );
    smol::block_on(validate_script_with(&script, CancellationToken::default())).unwrap();
    assert!(!marker.exists());
}

#[test]
fn generation_bounds_logs_and_generated_script() {
    let fixture = Fixture::new(
        good_response(),
        "head -c 150000 /dev/zero >&2; cp \"$root/response.json\" \"$result\"",
    );
    assert!(
        format!(
            "{:#}",
            fixture.generate(Duration::from_secs(3)).unwrap_err()
        )
        .contains("size limit")
    );
    let response =
        serde_json::json!({"summary":"Large", "script":"x".repeat(64 * 1024 + 1)}).to_string();
    let fixture = Fixture::new(&response, "cp \"$root/response.json\" \"$result\"");
    assert!(
        format!(
            "{:#}",
            fixture.generate(Duration::from_secs(3)).unwrap_err()
        )
        .contains("64 KiB")
    );
}

#[test]
fn timeout_kills_codex_and_its_tool_children() {
    let fixture = Fixture::new(
        good_response(),
        "sleep 30 &\nprintf '%s' \"$!\" > \"$root/child-pid\"\nwait",
    );
    let error = fixture.generate(Duration::from_millis(200)).unwrap_err();
    assert!(error.to_string().contains("timed out"));
    smol::block_on(async {
        assert_stopped(fixture.read("pid").parse().unwrap()).await;
        assert_stopped(fixture.read("child-pid").parse().unwrap()).await;
    });
}

#[test]
fn dropping_generation_kills_the_complete_process_group() {
    let fixture = Fixture::new(
        good_response(),
        "sleep 30 &\nprintf '%s' \"$!\" > \"$root/child-pid\"\nwait",
    );
    let program = fixture.program.clone();
    smol::block_on(async {
        let task = smol::spawn(async move {
            generate_with(
                program.as_os_str(),
                request(),
                String::new(),
                Duration::from_secs(30),
            )
            .await
        });
        let child = wait_for_pid(&fixture.root.path().join("child-pid")).await;
        task.cancel().await;
        assert_stopped(fixture.read("pid").parse().unwrap()).await;
        assert_stopped(child).await;
    });
}

#[test]
fn exited_codex_does_not_leave_children_holding_output_pipes() {
    let fixture = Fixture::new(
        good_response(),
        "cp \"$root/response.json\" \"$result\"\nsleep 30 &\nprintf '%s' \"$!\" > \"$root/child-pid\"\nexit 0",
    );
    fixture.generate(Duration::from_secs(3)).unwrap();
    smol::block_on(assert_stopped(fixture.read("child-pid").parse().unwrap()));
}

#[test]
fn cancellation_owner_stops_children_without_dropping_the_running_future() {
    let fixture = Fixture::new(
        good_response(),
        "sleep 30 &\nprintf '%s' \"$!\" > \"$root/child-pid\"\nwait",
    );
    let program = fixture.program.clone();
    let cancellation = GenerationCancellation::default();
    let token = cancellation.token();
    let worker_token = token.clone();
    smol::block_on(async {
        let task = smol::spawn(async move {
            super::generate_with(
                program.as_os_str(),
                request(),
                String::new(),
                Duration::from_secs(30),
                worker_token,
            )
            .await
        });
        let child = wait_for_pid(&fixture.root.path().join("child-pid")).await;
        let leader = fixture.read("pid").parse().unwrap();
        drop(cancellation);
        assert!(token.check().is_err());
        assert_stopped(leader).await;
        assert_stopped(child).await;
        assert!(task.await.is_err());
    });
}

#[test]
fn cancelled_owner_prevents_a_late_process_spawn() {
    let fixture = Fixture::new(good_response(), "cp \"$root/response.json\" \"$result\"");
    let cancellation = GenerationCancellation::default();
    let token = cancellation.token();
    drop(cancellation);
    let result = smol::block_on(super::generate_with(
        fixture.program.as_os_str(),
        request(),
        String::new(),
        Duration::from_secs(3),
        token,
    ));
    assert!(result.unwrap_err().to_string().contains("cancelled"));
    assert!(!fixture.root.path().join("pid").exists());
}
