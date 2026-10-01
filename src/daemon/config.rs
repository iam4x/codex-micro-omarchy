use super::*;
use crate::model::Profiles;

#[derive(PartialEq)]
enum Source {
    Missing,
    Content(String),
}

struct Watcher {
    path: PathBuf,
    loaded: Option<Source>,
}

impl Watcher {
    fn new(path: PathBuf) -> Self {
        Self { path, loaded: None }
    }

    fn poll(&mut self) -> Result<Option<Profile>> {
        let source = match fs::read_to_string(&self.path) {
            Ok(source) => Source::Content(source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Source::Missing,
            Err(error) => return Err(error).context("Could not read saved bindings"),
        };
        if self.loaded.as_ref() == Some(&source) {
            return Ok(None);
        }
        let profile = match &source {
            Source::Missing => Profiles::default().active().clone(),
            Source::Content(source) => Profiles::parse(source)?.active().clone(),
        };
        self.loaded = Some(source);
        Ok(Some(profile))
    }
}

fn refresh(watcher: &mut Watcher, hub: &SharedHub, last_error: &mut String) {
    match watcher.poll() {
        Ok(Some(profile)) => {
            hub.lock().unwrap().profile = profile;
            last_error.clear();
        }
        Ok(None) => last_error.clear(),
        Err(error) => {
            let message = format!("{error:#}");
            if *last_error != message {
                publish(hub, Event::ConfigError(message.clone()));
                *last_error = message;
            }
        }
    }
}

pub(super) fn start(hub: SharedHub) {
    let mut watcher = Watcher::new(config_path());
    let mut last_error = String::new();
    refresh(&mut watcher, &hub, &mut last_error);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(50));
            refresh(&mut watcher, &hub, &mut last_error);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Control, Phase};

    #[test]
    fn reloads_initial_changes_and_deletion_without_hardware() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut watcher = Watcher::new(path.clone());
        assert!(watcher.poll().unwrap().unwrap().bindings.is_empty());
        assert!(watcher.poll().unwrap().is_none());
        let mut profiles = Profiles::default();
        profiles.active_mut().assign(
            Control::AG00,
            Phase::Press,
            Some(Action::Text {
                text: "first".into(),
                submit: false,
            }),
        );
        profiles.save(&path).unwrap();
        assert_eq!(
            watcher.poll().unwrap().unwrap().bindings,
            profiles.active().bindings
        );
        assert!(watcher.poll().unwrap().is_none());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        profiles.active_mut().assign(
            Control::AG00,
            Phase::Press,
            Some(Action::Text {
                text: "other".into(),
                submit: false,
            }),
        );
        fs::write(&path, toml::to_string(&profiles).unwrap()).unwrap();
        fs::File::open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            watcher.poll().unwrap().unwrap().bindings,
            profiles.active().bindings
        );
        fs::remove_file(path).unwrap();
        assert!(watcher.poll().unwrap().unwrap().bindings.is_empty());
    }

    #[test]
    fn bad_edits_keep_the_last_valid_profile_and_recovery_loads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut profiles = Profiles::default();
        profiles.active_mut().assign(
            Control::Mic,
            Phase::Press,
            Some(Action::Text {
                text: "keep me".into(),
                submit: false,
            }),
        );
        profiles.save(&path).unwrap();
        let hub = Arc::new(Mutex::new(Hub::default()));
        let mut watcher = Watcher::new(path.clone());
        let mut error = String::new();
        refresh(&mut watcher, &hub, &mut error);
        fs::write(&path, "invalid = [").unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(!error.is_empty());
        assert_eq!(
            hub.lock().unwrap().profile.bindings,
            profiles.active().bindings
        );
        Profiles::default().save(&path).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(hub.lock().unwrap().profile.bindings.is_empty());
        assert!(error.is_empty());
    }
    #[test]
    fn activation_reloads_bindings_and_invalid_inactive_profiles_keep_last_valid() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut profiles = Profiles::default();
        let desktop_action = Action::Text {
            text: "desktop".into(),
            submit: false,
        };
        let work_action = Action::Text {
            text: "work".into(),
            submit: false,
        };
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Press, Some(desktop_action.clone()));
        let work = profiles.add("Work").unwrap();
        profiles.activate(work).unwrap();
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Press, Some(work_action.clone()));
        profiles.activate(0).unwrap();
        profiles.save(&path).unwrap();
        let hub = Arc::new(Mutex::new(Hub::default()));
        let mut watcher = Watcher::new(path.clone());
        let mut error = String::new();
        refresh(&mut watcher, &hub, &mut error);
        assert_eq!(
            hub.lock()
                .unwrap()
                .profile
                .action(Control::AG00, Phase::Press),
            Some(&desktop_action)
        );
        profiles.activate(work).unwrap();
        profiles.save(&path).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert_eq!(
            hub.lock()
                .unwrap()
                .profile
                .action(Control::AG00, Phase::Press),
            Some(&work_action)
        );
        profiles.activate(0).unwrap();
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Step, Some(desktop_action));
        profiles.activate(work).unwrap();
        fs::write(&path, toml::to_string(&profiles).unwrap()).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(!error.is_empty());
        assert_eq!(
            hub.lock()
                .unwrap()
                .profile
                .action(Control::AG00, Phase::Press),
            Some(&work_action)
        );
        profiles.activate(0).unwrap();
        profiles
            .active_mut()
            .assign(Control::AG00, Phase::Step, None);
        profiles.activate(work).unwrap();
        profiles.save(&path).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(error.is_empty());
        assert_eq!(
            hub.lock()
                .unwrap()
                .profile
                .action(Control::AG00, Phase::Press),
            Some(&work_action)
        );
        let empty = profiles.add("Empty").unwrap();
        profiles.activate(empty).unwrap();
        profiles.save(&path).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(hub.lock().unwrap().profile.bindings.is_empty());
    }
}
