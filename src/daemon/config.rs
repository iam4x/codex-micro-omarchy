use super::*;

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
            Source::Missing => Profile::default(),
            Source::Content(source) => Profile::parse(source)?,
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
        Ok(None) => {}
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
            thread::sleep(Duration::from_millis(400));
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
        let mut profile = Profile::default();
        profile.assign(
            Control::AG00,
            Phase::Press,
            Some(Action::Text {
                text: "first".into(),
                submit: false,
            }),
        );
        profile.save(&path).unwrap();
        assert_eq!(watcher.poll().unwrap().unwrap().bindings, profile.bindings);
        assert!(watcher.poll().unwrap().is_none());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        profile.assign(
            Control::AG00,
            Phase::Press,
            Some(Action::Text {
                text: "other".into(),
                submit: false,
            }),
        );
        fs::write(&path, toml::to_string(&profile).unwrap()).unwrap();
        fs::File::open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(watcher.poll().unwrap().unwrap().bindings, profile.bindings);
        fs::remove_file(path).unwrap();
        assert!(watcher.poll().unwrap().unwrap().bindings.is_empty());
    }

    #[test]
    fn bad_edits_keep_the_last_valid_profile_and_recovery_loads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bindings.toml");
        let mut profile = Profile::default();
        profile.assign(
            Control::Mic,
            Phase::Press,
            Some(Action::Text {
                text: "keep me".into(),
                submit: false,
            }),
        );
        profile.save(&path).unwrap();
        let hub = Arc::new(Mutex::new(Hub::default()));
        let mut watcher = Watcher::new(path.clone());
        let mut error = String::new();
        refresh(&mut watcher, &hub, &mut error);
        fs::write(&path, "invalid = [").unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(!error.is_empty());
        assert_eq!(hub.lock().unwrap().profile.bindings, profile.bindings);
        Profile::default().save(&path).unwrap();
        refresh(&mut watcher, &hub, &mut error);
        assert!(hub.lock().unwrap().profile.bindings.is_empty());
        assert!(error.is_empty());
    }
}
