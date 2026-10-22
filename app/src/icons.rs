#[cfg(target_os = "linux")]
fn resolve_executable(command: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(command);
    if path.is_absolute() {
        return path.canonicalize().ok();
    }
    if path.components().count() != 1 {
        return None;
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .find_map(|folder| folder.join(command).canonicalize().ok())
}
#[cfg(target_os = "linux")]
fn desktop_values(text: &str) -> std::collections::HashMap<&str, &str> {
    let mut entry = false;
    let mut values = std::collections::HashMap::new();
    for line in text.lines() {
        if line.starts_with('[') {
            entry = line == "[Desktop Entry]";
            continue;
        }
        if entry {
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim(), value.trim());
            }
        }
    }
    values
}
#[cfg(target_os = "linux")]
fn desktop_executable(
    values: &std::collections::HashMap<&str, &str>,
) -> Option<std::path::PathBuf> {
    let tokens = shlex::split(values.get("Exec")?)?;
    let mut arguments = tokens.iter();
    let mut executable = arguments.next()?;
    if std::path::Path::new(executable)
        .file_name()
        .is_some_and(|name| name == "env")
    {
        executable =
            arguments.find(|argument| !argument.contains('=') && !argument.starts_with('-'))?;
    }
    resolve_executable(executable)
}

/// The parts of a `.desktop` file needed to match a running process to its icon.
/// The executable is resolved through PATH once, when the entry is parsed.
#[cfg(target_os = "linux")]
struct DesktopEntry {
    executable: Option<std::path::PathBuf>,
    id: String,
    icon: Option<String>,
    name: Option<String>,
    wm_class: Option<String>,
    /// Shown in launchers: an Application that is neither NoDisplay nor Hidden.
    listed: bool,
}
#[cfg(target_os = "linux")]
impl DesktopEntry {
    fn parse(text: &str) -> Option<Self> {
        let values = desktop_values(text);
        values.get("Exec")?;
        Some(Self {
            executable: desktop_executable(&values),
            id: String::new(),
            icon: values.get("Icon").map(|value| (*value).to_owned()),
            name: values.get("Name").map(|value| (*value).to_owned()),
            wm_class: values
                .get("StartupWMClass")
                .map(|value| (*value).to_owned())
                .or_else(|| {
                    shlex::split(values.get("Exec")?)?
                        .into_iter()
                        .find_map(|arg| {
                            let id = arg.strip_prefix("steam://rungameid/")?;
                            (!id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()))
                                .then(|| format!("steam_app_{id}"))
                        })
                }),
            listed: values.get("NoDisplay") != Some(&"true")
                && values.get("Hidden") != Some(&"true")
                && values.get("Type").is_none_or(|kind| *kind == "Application"),
        })
    }
    /// `process` must already be canonical.
    fn icon_for(&self, process: &std::path::Path, app_name: &str) -> Option<(u8, String)> {
        if self.executable.as_deref() != Some(process) {
            return None;
        }
        let icon = self.icon.clone()?;
        let score = [self.name.as_deref(), self.wm_class.as_deref()]
            .into_iter()
            .flatten()
            .map(|candidate| name_match_score(app_name, candidate))
            .max()
            .unwrap_or_default();
        Some((score, icon))
    }
}
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
fn desktop_icon(text: &str, process: &std::path::Path, app_name: &str) -> Option<(u8, String)> {
    DesktopEntry::parse(text)?.icon_for(&process.canonicalize().ok()?, app_name)
}
#[cfg(target_os = "linux")]
fn name_match_score(app_name: &str, candidate: &str) -> u8 {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let name = normalize(app_name);
    let candidate = normalize(candidate);
    if name.is_empty() || candidate.len() < 3 {
        0
    } else if name == candidate {
        100
    } else if name.starts_with(&candidate) {
        90
    } else if name.contains(&candidate) {
        80
    } else {
        0
    }
}
#[cfg(target_os = "linux")]
#[cfg_attr(not(test), allow(dead_code))]
fn choose_desktop_icon(
    entries: &[String],
    process: &std::path::Path,
    app_name: &str,
) -> Option<String> {
    let entries: Vec<_> = entries
        .iter()
        .map(String::as_str)
        .filter_map(DesktopEntry::parse)
        .collect();
    choose_entry_icon(&entries, process, app_name)
}
#[cfg(target_os = "linux")]
fn choose_entry_icon(
    entries: &[DesktopEntry],
    process: &std::path::Path,
    app_name: &str,
) -> Option<String> {
    let process = process.canonicalize().ok()?;
    let matches: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.icon_for(&process, app_name))
        .collect();
    if matches.len() == 1 {
        return matches.into_iter().next().map(|(_, icon)| icon);
    }
    let best_score = matches.iter().map(|(score, _)| *score).max()?;
    if best_score == 0
        || matches
            .iter()
            .filter(|(score, _)| *score == best_score)
            .count()
            != 1
    {
        return None;
    }
    matches
        .into_iter()
        .find(|(score, _)| *score == best_score)
        .map(|(_, icon)| icon)
}
#[cfg(target_os = "linux")]
fn application_directories() -> Vec<std::path::PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        directories.push(home.join(".local/share/applications"));
        directories.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        directories.insert(0, std::path::PathBuf::from(data).join("applications"));
    }
    directories.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share:/var/lib/flatpak/exports/share".into())
            .split(':')
            .map(|directory| std::path::PathBuf::from(directory).join("applications")),
    );
    directories
}
#[cfg(target_os = "linux")]
fn named_entry<'a>(entries: &'a [DesktopEntry], name: &str) -> Option<&'a DesktopEntry> {
    // Desktop IDs take precedence over shared executables such as Steam and Flatpak.
    entries
        .iter()
        .find(|entry| {
            !entry.id.is_empty()
                && (entry.id.eq_ignore_ascii_case(name)
                    || name
                        .strip_suffix(".desktop")
                        .is_some_and(|id| entry.id.eq_ignore_ascii_case(id)))
        })
        .or_else(|| {
            let mut matches = entries.iter().filter(|entry| {
                [entry.wm_class.as_deref(), entry.name.as_deref()]
                    .into_iter()
                    .flatten()
                    .any(|value| value.eq_ignore_ascii_case(name))
            });
            let entry = matches.next()?;
            matches.next().is_none().then_some(entry)
        })
}

pub fn application_name(name: &str) -> String {
    #[cfg(target_os = "linux")]
    if let Some(name) = named_entry(&desktop_index(), name).and_then(|entry| entry.name.clone()) {
        return name;
    }
    name.into()
}

/// Every `.desktop` entry, read once and reused by all icon lookups. Rebuilt after
/// a few minutes so newly installed applications are picked up. Previously each
/// cache miss re-read and re-resolved every entry, which made startup quadratic in
/// the number of installed applications.
#[cfg(target_os = "linux")]
fn desktop_index() -> std::sync::Arc<Vec<DesktopEntry>> {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    static INDEX: Mutex<Option<(Instant, Arc<Vec<DesktopEntry>>)>> = Mutex::new(None);
    let mut index = INDEX.lock().unwrap();
    if let Some((built, entries)) = index.as_ref() {
        if built.elapsed() < Duration::from_secs(300) {
            return entries.clone();
        }
    }
    let entries: Arc<Vec<_>> = Arc::new(
        application_directories()
            .into_iter()
            .flat_map(|directory| {
                let mut files: Vec<_> = std::fs::read_dir(directory)
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .collect();
                files.sort_by_key(|file| file.file_name());
                files
            })
            .filter(|file| {
                file.path()
                    .extension()
                    .is_some_and(|extension| extension == "desktop")
            })
            .filter_map(|file| {
                let mut entry = DesktopEntry::parse(&std::fs::read_to_string(file.path()).ok()?)?;
                entry.id = file.path().file_stem()?.to_string_lossy().into_owned();
                Some(entry)
            })
            .collect(),
    );
    *index = Some((Instant::now(), entries.clone()));
    entries
}
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn preview_application_processes() -> Vec<(String, String)> {
    let mut candidates = Vec::new();
    for entry in desktop_index().iter().filter(|entry| entry.listed) {
        let Some(app_name) = entry.name.clone() else {
            continue;
        };
        let candidate = (
            match entry.executable.as_ref() {
                Some(exe) => exe.to_string_lossy().into_owned(),
                None => continue,
            },
            app_name,
        );
        if candidates.contains(&candidate) || SystemIcons.icon(&candidate.0, &candidate.1).is_none()
        {
            continue;
        }
        candidates.push(candidate);
        if candidates.len() == 5 {
            break;
        }
    }
    candidates
}
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn preview_application_processes() -> Vec<(String, String)> {
    Vec::new()
}

#[derive(Clone, Debug)]
pub enum IconData {
    Png(Vec<u8>),
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Svg(Vec<u8>),
}

trait IconProvider {
    fn icon(&self, process: &str, app_name: &str) -> Option<IconData>;
}
struct SystemIcons;
#[cfg(target_os = "linux")]
impl IconProvider for SystemIcons {
    fn icon(&self, app: &str, app_name: &str) -> Option<IconData> {
        let entries = desktop_index();
        let icon = named_entry(&entries, app_name)
            .or_else(|| named_entry(&entries, app))
            .and_then(|entry| entry.icon.clone())
            .or_else(|| choose_entry_icon(&entries, std::path::Path::new(app), app_name))?;
        let path = if std::path::Path::new(&icon).is_absolute() {
            std::path::PathBuf::from(icon)
        } else {
            freedesktop_icons::lookup(&icon)
                .with_size(32)
                .with_theme("breeze")
                .with_cache()
                .find()
                .or_else(|| {
                    freedesktop_icons::lookup(&icon)
                        .with_size(32)
                        .with_cache()
                        .find()
                })?
        };
        let svg = match path.extension()?.to_str()? {
            "svg" => true,
            "png" => false,
            "xpm" => return None,
            _ => return None,
        };
        if std::fs::metadata(&path).ok()?.len() > 2_000_000 {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        Some(if svg {
            IconData::Svg(bytes)
        } else {
            IconData::Png(bytes)
        })
    }
}
#[cfg(not(target_os = "linux"))]
impl IconProvider for SystemIcons {
    fn icon(&self, process: &str, app_name: &str) -> Option<IconData> {
        #[cfg(target_os = "windows")]
        return windows_icon(process, app_name);
        #[cfg(target_os = "macos")]
        return macos_icon(process, app_name);
        #[allow(unreachable_code)]
        let _ = (process, app_name);
        None
    }
}

pub fn application_icons(
    apps: Vec<(String, String)>,
) -> std::collections::HashMap<String, Option<IconData>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, bool>>> =
        std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    let mut result = std::collections::HashMap::new();
    for (process, app_name) in apps.into_iter().take(200) {
        if process.is_empty()
            || process.len() > 4096
            || process.contains('\0')
            || app_name.len() > 1024
        {
            continue;
        }
        if cache.len() > 1000 {
            cache.clear();
        }
        let key = icon_key(&process, &app_name);
        let icon = if cache.get(&key) == Some(&false) {
            None
        } else {
            SystemIcons.icon(&process, &app_name)
        };
        cache.insert(key.clone(), icon.is_some());
        result.insert(key, icon);
    }
    result
}

#[cfg(target_os = "windows")]
fn windows_icon(process: &str, _app_name: &str) -> Option<IconData> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
            HGDIOBJ,
        },
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    };

    struct Resources {
        icon: HICON,
        screen: HDC,
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
    }
    impl Drop for Resources {
        fn drop(&mut self) {
            unsafe {
                if !self.previous.is_null() && !self.dc.is_null() {
                    SelectObject(self.dc, self.previous);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                if !self.dc.is_null() {
                    DeleteDC(self.dc);
                }
                if !self.screen.is_null() {
                    ReleaseDC(null_mut(), self.screen);
                }
                if !self.icon.is_null() {
                    DestroyIcon(self.icon);
                }
            }
        }
    }

    let wide: Vec<u16> = std::ffi::OsStr::new(process)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHFILEINFOW::default();
    let found = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    let mut resources = Resources {
        icon: info.hIcon,
        screen: null_mut(),
        dc: null_mut(),
        bitmap: null_mut(),
        previous: null_mut(),
    };
    let bytes = (|| {
        let screen = unsafe { GetDC(null_mut()) };
        if screen.is_null() {
            return None;
        }
        resources.screen = screen;
        let dc = unsafe { CreateCompatibleDC(screen) };
        if dc.is_null() {
            return None;
        }
        resources.dc = dc;
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 32 * 32 * 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = null_mut();
        resources.bitmap = unsafe {
            CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0)
        };
        if resources.bitmap.is_null() || pixels.is_null() {
            return None;
        }
        resources.previous = unsafe { SelectObject(dc, resources.bitmap) };
        if resources.previous.is_null()
            || unsafe { DrawIconEx(dc, 0, 0, resources.icon, 32, 32, 0, null_mut(), DI_NORMAL) }
                == 0
        {
            return None;
        }
        let bgra = unsafe { std::slice::from_raw_parts(pixels.cast::<u8>(), 32 * 32 * 4) };
        Some(bgra_to_rgba(bgra))
    })()?;
    encode_png(&bytes, 32, 32)
}

#[cfg(target_os = "macos")]
fn macos_icon(process: &str, _app_name: &str) -> Option<IconData> {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSString};
    use std::ptr::NonNull;

    let process_path = std::path::Path::new(process);
    let icon_path = process_path
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .unwrap_or(process_path);
    let image = NSWorkspace::sharedWorkspace()
        .iconForFile(&NSString::from_str(&icon_path.to_string_lossy()));
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::new();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
    }?;
    let length = png.length();
    if length == 0 || length > 2_000_000 {
        return None;
    }
    let mut bytes = vec![0; length];
    let buffer = NonNull::new(bytes.as_mut_ptr().cast())?;
    unsafe { png.getBytes_length(buffer, length as _) };
    Some(IconData::Png(bytes))
}

#[cfg(target_os = "windows")]
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Option<IconData> {
    let mut png_bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().ok()?.write_image_data(rgba).ok()?;
    Some(IconData::Png(png_bytes))
}

fn icon_key(process: &str, app_name: &str) -> String {
    format!("{process}\0{app_name}")
}

#[cfg(any(target_os = "windows", test))]
fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let pixels = bgra.as_chunks::<4>().0;
    let alpha_present = pixels.iter().any(|pixel| pixel[3] != 0);
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        rgba.extend_from_slice(&[
            pixel[2],
            pixel[1],
            pixel[0],
            if alpha_present { pixel[3] } else { 255 },
        ]);
    }
    rgba
}

#[cfg(test)]
mod pixel_tests {
    #[test]
    fn windows_icon_pixels_are_converted_to_rgba() {
        assert_eq!(super::bgra_to_rgba(&[3, 2, 1, 0]), [1, 2, 3, 255]);
        assert_eq!(super::bgra_to_rgba(&[3, 2, 1, 128]), [1, 2, 3, 128]);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn desktop_ids_and_steam_classes_have_names_and_icons_without_live_processes() {
        let mut entry = super::DesktopEntry::parse(
            "[Desktop Entry]\nName=Gwenview\nIcon=gwenview\nExec=/definitely/not/installed\nStartupWMClass=org.kde.gwenview"
        ).unwrap();
        entry.id = "org.kde.gwenview".into();
        let entries = vec![entry];
        let matched = super::named_entry(&entries, "org.kde.gwenview").unwrap();
        assert_eq!(matched.name.as_deref(), Some("Gwenview"));
        assert_eq!(matched.icon.as_deref(), Some("gwenview"));
        assert!(super::named_entry(&entries, "org.kde.other").is_none());
        let mut game = super::DesktopEntry::parse(
            "[Desktop Entry]\nName=Game\nIcon=steam_icon_1449690\nExec=steam steam://rungameid/1449690"
        ).unwrap();
        game.id = "Game".into();
        assert_eq!(
            super::named_entry(&[game], "steam_app_1449690")
                .unwrap()
                .name
                .as_deref(),
            Some("Game")
        );
    }

    #[test]
    #[ignore = "requires the reported applications installed on Linux"]
    fn reported_linux_applications_have_launcher_names_and_icons() {
        use super::IconProvider;
        for (id, expected) in [
            ("org.kde.gwenview", "Gwenview"),
            ("org.telegram.desktop", "Telegram"),
            ("firefox", "Firefox"),
            (
                "steam_app_1449690",
                "The Walking Dead: The Telltale Definitive Series",
            ),
        ] {
            assert_eq!(super::application_name(id), expected);
            assert!(
                super::SystemIcons.icon(id, id).is_some(),
                "missing icon for {id}"
            );
        }
    }

    #[test]
    #[ignore = "requires installed Linux applications and icon themes"]
    fn preview_processes_resolve_to_application_icons() {
        let apps = super::preview_application_processes();
        assert!(
            !apps.is_empty(),
            "no installed desktop apps were discovered"
        );
        assert!(apps
            .iter()
            .all(|(process, _)| std::path::Path::new(process).is_absolute()));
        let icons = super::application_icons(apps);
        assert!(icons.values().all(Option::is_some));
    }

    #[test]
    fn desktop_icon_matches_exact_process_executable_not_labels() {
        let exe = std::env::current_exe().unwrap();
        let text=format!("[Desktop Entry]\nName=Wrong name\nStartupWMClass=WrongClass\nIcon=correct\nExec=\"{}\" %u\n[Desktop Action New]\nIcon=wrong",exe.display());
        assert_eq!(
            super::desktop_icon(&text, &exe, "WrongClass"),
            Some((100, "correct".into()))
        );
        assert_eq!(
            super::desktop_icon(&text, std::path::Path::new("/missing/other"), "WrongClass"),
            None
        );
        assert_eq!(
            super::desktop_icon(
                "[Desktop Entry]\nName=editor\nStartupWMClass=editor\nIcon=wrong",
                &exe,
                "WrongClass"
            ),
            None
        );
    }

    #[test]
    fn desktop_entries_resolve_executable_and_launcher_visibility_once() {
        let exe = std::env::current_exe().unwrap();
        let entry = |extra: &str| {
            format!(
                "[Desktop Entry]\nType=Application\nName=App\nIcon=app\nExec=\"{}\" %U\n{extra}",
                exe.display()
            )
        };
        let shown = super::DesktopEntry::parse(&entry("")).unwrap();
        assert!(shown.listed);
        assert_eq!(shown.executable, Some(exe.canonicalize().unwrap()));
        assert_eq!(
            shown.icon_for(&exe.canonicalize().unwrap(), "App"),
            Some((100, "app".into()))
        );
        assert!(
            !super::DesktopEntry::parse(&entry("NoDisplay=true"))
                .unwrap()
                .listed
        );
        assert!(
            !super::DesktopEntry::parse(&entry("Hidden=true"))
                .unwrap()
                .listed
        );
        assert!(super::DesktopEntry::parse("[Desktop Entry]\nName=No command").is_none());
    }

    #[test]
    fn launcher_icons_are_disambiguated_by_application_name() {
        let exe = std::env::current_exe().unwrap();
        let entry = |name: &str, icon: &str| {
            format!(
                "[Desktop Entry]\nName={name}\nStartupWMClass={name}\nIcon={icon}\nExec=\"{}\"\n",
                exe.display()
            )
        };
        assert_eq!(
            super::choose_desktop_icon(
                &[entry("Wrong Game", "wrong"), entry("Right Game", "right")],
                &exe,
                "Right Game",
            ),
            Some("right".into())
        );
        assert_eq!(
            super::choose_desktop_icon(
                &[entry("Wrong Game", "wrong"), entry("Right Game", "right")],
                &exe,
                "Unknown Window",
            ),
            None
        );
    }
}
#[cfg(all(test, target_os = "linux"))]
mod desktop_tests {
    #[test]
    #[ignore = "requires installed KDE applications and icon themes"]
    fn kde_application_icons_resolve_to_local_images() {
        let apps = ["Dolphin", "Konsole"].map(|name| {
            (
                super::resolve_executable(name.to_lowercase().as_str())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                name.into(),
            )
        });
        let icons = super::application_icons(apps.into());
        assert!(icons.values().all(|icon| icon
            .as_ref()
            .is_some_and(|icon| matches!(icon, super::IconData::Png(bytes) | super::IconData::Svg(bytes) if !bytes.is_empty()))));
    }
}
