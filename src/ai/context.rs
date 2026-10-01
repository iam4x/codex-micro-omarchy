use super::{CancellationToken, bounded_file, run_process};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, process::Command, time::Duration};

#[derive(Deserialize, serde::Serialize)]
struct Window {
    class: String,
    address: String,
}

pub(super) async fn gather(cancellation: CancellationToken) -> String {
    let local = smol::unblock(installed_apps).await;
    let mut result = String::new();
    for args in [&["version"][..], &["-j", "clients"][..]] {
        let mut command = Command::new("hyprctl");
        command.args(args);
        let response = smol::future::or(run_process(command, &[], cancellation.clone()), async {
            smol::Timer::after(Duration::from_secs(2)).await;
            anyhow::bail!("Desktop inspection timed out")
        })
        .await;
        match response {
            Ok(output) if output.status.success() && args == ["version"] => {
                result.push_str("\nHyprland version:\n");
                result.push_str(&String::from_utf8_lossy(&output.stdout));
            }
            Ok(output) if output.status.success() => {
                if let Ok(windows) = serde_json::from_slice::<Vec<Window>>(&output.stdout) {
                    let windows: Vec<_> = windows.into_iter().take(100).collect();
                    result.push_str("\nRunning window classes and addresses:\n");
                    result.push_str(&serde_json::to_string(&windows).unwrap_or_default());
                }
            }
            _ => result.push_str(
                "\nHyprland inspection unavailable. Discover commands read-only if needed.\n",
            ),
        }
    }
    result.push_str("\nCurrent Lua focus dispatch example: hyprctl dispatch 'hl.dsp.focus({window=\"address:0x123\"})'. Do not assume legacy dispatcher syntax. Check installed API read-only.\n");
    result.push_str(&local);
    let maximum = 48 * 1024;
    if result.len() > maximum {
        let end = result
            .char_indices()
            .take_while(|(index, _)| *index <= maximum)
            .last()
            .map(|(index, _)| index)
            .unwrap_or(0);
        result.truncate(end);
    }
    result
}

fn installed_apps() -> String {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let mut entries = BTreeMap::new();
    for root in [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        data.join("applications"),
    ] {
        let Ok(files) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in files.flatten().take(400) {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "desktop")
            {
                continue;
            }
            let Some(source) = bounded_file(&path, 64 * 1024) else {
                continue;
            };
            apply_desktop_entry(
                &mut entries,
                entry.file_name().to_string_lossy().into_owned(),
                &source,
            );
        }
    }
    let mut result = String::from(
        "Installed desktop launchers (desktop Exec fields use desktop-entry escaping and field codes):\n",
    );
    for (id, (name, exec)) in entries {
        result.push_str(&format!("{id}: {name}; Exec={exec}\n"));
        if result.len() > 40 * 1024 {
            break;
        }
    }
    let mut executables = std::collections::BTreeSet::new();
    if let Some(path) = std::env::var_os("PATH") {
        for root in std::env::split_paths(&path) {
            let Ok(files) = std::fs::read_dir(root) else {
                continue;
            };
            for entry in files.flatten().take(1000) {
                if executables.len() >= 1200 {
                    break;
                }
                use std::os::unix::fs::PermissionsExt;
                if entry.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                }) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !name.starts_with('.') {
                        executables.insert(name);
                    }
                }
            }
        }
    }
    result.push_str("\nAvailable executable names (partial):\n");
    result.push_str(&executables.into_iter().collect::<Vec<_>>().join(", "));
    result
}

fn apply_desktop_entry(entries: &mut BTreeMap<String, (String, String)>, id: String, source: &str) {
    let mut section = false;
    let mut name = None;
    let mut exec = None;
    let mut hidden = false;
    for line in source.lines() {
        if line.starts_with('[') {
            section = line == "[Desktop Entry]";
            continue;
        }
        if !section {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name = Some(value.to_owned());
        }
        if let Some(value) = line.strip_prefix("Exec=") {
            exec = Some(value.to_owned());
        }
        hidden |= line == "Hidden=true";
    }
    if hidden {
        entries.remove(&id);
    } else if let (Some(name), Some(exec)) = (name, exec) {
        entries.insert(id, (name, exec));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_user_launcher_removes_the_system_entry_with_same_id() {
        let mut entries = BTreeMap::new();
        apply_desktop_entry(
            &mut entries,
            "app.desktop".into(),
            "[Desktop Entry]\nName=System\nExec=system-app\n",
        );
        apply_desktop_entry(
            &mut entries,
            "app.desktop".into(),
            "[Desktop Entry]\nHidden=true\n",
        );
        assert!(entries.is_empty());
        apply_desktop_entry(
            &mut entries,
            "app.desktop".into(),
            "[Desktop Entry]\nName=User\nExec=user-app\n",
        );
        assert_eq!(entries["app.desktop"], ("User".into(), "user-app".into()));
    }
}
