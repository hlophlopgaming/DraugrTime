use serde::{Deserialize, Serialize};
use std::{io, path::Path};

/// Stored in place of the real window title for applications in
/// [`Preferences::masked_apps`]. Fixed and language-independent so stored data
/// stays stable when the UI language changes.
pub const MASKED_TITLE: &str = "[masked]";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub onboarded: bool,
    pub excluded_apps: Vec<String>,
    /// Applications whose time is recorded but whose window titles are
    /// replaced with [`MASKED_TITLE`] before storage. Matched like
    /// `excluded_apps`; exclusion wins when an app is in both lists.
    pub masked_apps: Vec<String>,
    pub schedule: Option<RecordingSchedule>,
}
impl Preferences {
    fn validate(&self) -> io::Result<()> {
        if self
            .schedule
            .as_ref()
            .is_some_and(|schedule| !schedule.valid())
        {
            return Err(io::Error::other("Invalid recording schedule"));
        }
        Ok(())
    }
    pub fn load(path: &Path) -> io::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let preferences: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                preferences.validate()?;
                Ok(preferences)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("invalid preferences path"))?;
        std::fs::create_dir_all(parent)?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        use io::Write;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(io::Error::other)?;
        Ok(())
    }
    pub fn filter(
        &self,
        window: Option<draugr_time_watchers::WindowInfo>,
    ) -> Option<draugr_time_watchers::WindowInfo> {
        let mut window = window?;
        let listed = |apps: &[String]| apps.iter().any(|app| app.eq_ignore_ascii_case(&window.app));
        if listed(&self.excluded_apps) {
            return None;
        }
        if listed(&self.masked_apps) {
            window.title = MASKED_TITLE.to_owned();
        }
        Some(window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use draugr_time_watchers::WindowInfo;
    #[test]
    fn privacy_filters_and_preferences_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut prefs = Preferences::load(&path).unwrap();
        assert!(!prefs.onboarded);
        prefs.excluded_apps.push("Secret".into());
        prefs.onboarded = true;
        prefs.save(&path).unwrap();
        prefs.save(&path).unwrap();
        let prefs = Preferences::load(&path).unwrap();
        assert!(prefs.onboarded);
        let window = |app: &str| {
            Some(WindowInfo {
                app: app.into(),
                title: "private".into(),
                pid: None,
                app_name: String::new(),
            })
        };
        assert!(prefs.filter(window("secret")).is_none());
        assert_eq!(prefs.filter(window("editor")).unwrap().title, "private");
        let mut prefs = prefs;
        prefs.masked_apps.push("Editor".into());
        let masked = prefs.filter(window("editor")).unwrap();
        assert_eq!(
            (masked.app.as_str(), masked.title.as_str()),
            ("editor", MASKED_TITLE)
        );
        std::fs::write(&path, "broken").unwrap();
        assert!(Preferences::load(&path).is_err());
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingSchedule {
    /// ISO weekdays: Monday=1 through Sunday=7; local wall-clock minutes.
    pub weekdays: Vec<u32>,
    pub start_minute: u32,
    pub end_minute: u32,
}
impl RecordingSchedule {
    pub fn allows(&self, weekday: u32, minute: u32) -> bool {
        self.weekdays.contains(&weekday) && minute >= self.start_minute && minute < self.end_minute
    }
    fn valid(&self) -> bool {
        !self.weekdays.is_empty()
            && self.weekdays.iter().all(|day| (1..=7).contains(day))
            && self.start_minute < self.end_minute
            && self.end_minute <= 1440
    }
}
