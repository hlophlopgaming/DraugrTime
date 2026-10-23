use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    Ru,
    De,
    Fr,
    Es,
    Pt,
    Zh,
    Ja,
}
impl Language {
    pub const ALL: [Self; 8] = [
        Self::En,
        Self::Ru,
        Self::De,
        Self::Fr,
        Self::Es,
        Self::Pt,
        Self::Zh,
        Self::Ja,
    ];
    pub fn code(self) -> &'static str {
        ["en", "ru", "de", "fr", "es", "pt", "zh", "ja"][self as usize]
    }
    pub fn name(self) -> &'static str {
        [
            "English",
            "Русский",
            "Deutsch",
            "Français",
            "Español",
            "Português",
            "简体中文",
            "日本語",
        ][self as usize]
    }
    pub fn detect(locale: &str) -> Self {
        let locale = locale.to_ascii_lowercase().replace('_', "-");
        if locale.starts_with("zh")
            && (locale.contains("tw") || locale.contains("hk") || locale.contains("hant"))
        {
            return Self::En;
        }
        Self::ALL
            .into_iter()
            .find(|l| locale.split(['-', '.', '@']).next() == Some(l.code()))
            .unwrap_or(Self::En)
    }
}
static LANGUAGE: AtomicUsize = AtomicUsize::new(0);
pub fn current() -> Language {
    Language::ALL[LANGUAGE.load(Ordering::Relaxed)]
}
pub fn set(language: Language) {
    LANGUAGE.store(language as usize, Ordering::Relaxed);
}
pub fn resolve(choice: Option<Language>, locale: &str) -> Language {
    choice.unwrap_or_else(|| Language::detect(locale))
}
pub fn catalogs() -> &'static serde_json::Value {
    static CATALOGS: OnceLock<serde_json::Value> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        serde_json::from_str(include_str!("../../translations.json"))
            .expect("valid bundled translations")
    })
}
pub fn translate(language: Language, key: &str) -> String {
    catalogs()[language.code()][key]
        .as_str()
        .unwrap_or(key)
        .to_owned()
}
pub fn tr(key: impl AsRef<str>) -> String {
    translate(current(), key.as_ref())
}
pub fn message(key: &str, values: &[(&str, String)]) -> String {
    let mut result = tr(key);
    for (name, value) in values {
        result = result.replace(&format!("{{{name}}}"), value);
    }
    result
}
pub fn load(path: &Path) -> std::io::Result<Option<Language>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let code: Option<String> =
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            Ok(code.and_then(|code| Language::ALL.into_iter().find(|l| l.code() == code)))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
pub fn save(path: &Path, choice: Option<Language>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec(&choice.map(Language::code)).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(temp, path)
}
pub fn path() -> std::io::Result<std::path::PathBuf> {
    // Keep UI preferences independent of the chosen history folder.
    Ok(draugr_time_daemon::paths::default_dir()?.join("language.json"))
}
pub trait LocaleProvider {
    fn locale(&self) -> String;
}
pub struct SystemLocale;
impl LocaleProvider for SystemLocale {
    fn locale(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn GetUserDefaultUILanguage() -> u16;
                fn LCIDToLocaleName(locale: u32, name: *mut u16, length: i32, flags: u32) -> i32;
            }
            let mut name = [0u16; 85];
            let length = unsafe {
                LCIDToLocaleName(
                    u32::from(GetUserDefaultUILanguage()),
                    name.as_mut_ptr(),
                    name.len() as i32,
                    0,
                )
            };
            if length > 1 {
                String::from_utf16_lossy(&name[..length as usize - 1])
            } else {
                String::new()
            }
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("/usr/bin/defaults")
                .args(["read", "-g", "AppleLanguages"])
                .output()
            {
                let value = String::from_utf8_lossy(&output.stdout);
                if let Some(locale) = value
                    .split(['(', ')', ',', '"', '\n'])
                    .map(str::trim)
                    .find(|s| !s.is_empty())
                {
                    return locale.into();
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
            .unwrap_or_default()
    }
}

pub fn segment_label(segment: &super::model::Segment) -> String {
    if matches!(segment.bucket, "computer" | "afk" | "derived") {
        tr(&segment.label)
    } else {
        segment.label.to_string()
    }
}
pub fn error(value: &str) -> String {
    let value = value.strip_prefix("invalid record: ").unwrap_or(value);
    if let Some((backend, details)) = value
        .split_once(": ")
        .filter(|(backend, _)| matches!(*backend, "window" | "idle"))
    {
        return format!("{}: {}", tr(backend), error(details));
    }
    for (prefix, key) in [
        ("permission required: ", "Permission required: {details}"),
        ("unsupported: ", "Unsupported: {details}"),
        ("platform error: ", "System error: {details}"),
    ] {
        if let Some(details) = value.strip_prefix(prefix) {
            return message(key, &[("details", tr(details))]);
        }
    }
    tr(value)
}

pub fn duration_in(language: Language, ms: i64) -> String {
    let seconds = ms.max(0) / 1000;
    let minutes = seconds / 60;
    if minutes >= 60 {
        format!(
            "{}{} {}{}",
            minutes / 60,
            translate(language, "h"),
            minutes % 60,
            translate(language, "m")
        )
    } else if minutes > 0 {
        format!("{minutes}{}", translate(language, "m"))
    } else {
        format!("{seconds}{}", translate(language, "s"))
    }
}
pub fn state(value: &str) -> String {
    tr(match value {
        "awake" => "Awake",
        "asleep" => "Sleep",
        "off" => "Off (OS evidence)",
        "active" => "Active",
        "away" => "AFK",
        "recording" => "Tracking",
        "paused" => "Paused",
        "stopped" => "Tracker stopped",
        "unobserved" => "Unobserved",
        "unknown" => "Unknown",
        "ok" => "Available",
        "permission_missing" | "permission_required" => "Permission missing",
        "unsupported" => "Unsupported",
        _ => value,
    })
}

pub fn short_date_label(date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let weekday = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][date.weekday().num_days_from_monday() as usize];
    let month = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][date.month0() as usize];
    format!(
        "{} {} {}",
        tr(weekday).chars().take(3).collect::<String>(),
        date.day(),
        tr(month).chars().take(3).collect::<String>()
    )
}
