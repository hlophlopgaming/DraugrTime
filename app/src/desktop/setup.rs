#[cfg(any(target_os = "linux", test))]
use std::path::Path;

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug)]
pub enum Bridge {
    Gnome,
    Kde,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub trait SetupProvider {
    #[cfg(target_os = "linux")]
    fn install_bridge(&self, bridge: Bridge) -> Result<(), String>;
    #[cfg(target_os = "linux")]
    fn install_desktop(&self) -> Result<(), String>;
    #[cfg(target_os = "macos")]
    fn open_permissions(&self) -> Result<(), String>;
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub struct SystemSetup;

#[cfg(any(target_os = "linux", test))]
fn install(root: &Path, bridge: Bridge) -> Result<(), String> {
    let files: &[(&str, &str)] = match bridge {
        Bridge::Gnome => &[
            (
                "gnome-shell/extensions/dagdatime@dagdatime.org/metadata.json",
                include_str!("../../../watchers/wayland/gnome/metadata.json"),
            ),
            (
                "gnome-shell/extensions/dagdatime@dagdatime.org/extension.js",
                include_str!("../../../watchers/wayland/gnome/extension.js"),
            ),
        ],
        Bridge::Kde => &[
            (
                "kwin/scripts/dagdatime/metadata.json",
                include_str!("../../../watchers/wayland/kde/metadata.json"),
            ),
            (
                "kwin/scripts/dagdatime/contents/ui/main.qml",
                include_str!("../../../watchers/wayland/kde/contents/ui/main.qml"),
            ),
        ],
    };
    for (relative, contents) in files {
        let file = root.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(file, contents).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl SetupProvider for SystemSetup {
    #[cfg(target_os = "linux")]
    fn install_bridge(&self, bridge: Bridge) -> Result<(), String> {
        let directory = draugr_time_daemon::paths::default_dir().map_err(|e| e.to_string())?;
        install(
            directory
                .parent()
                .ok_or("Cannot find local data directory")?,
            bridge,
        )
    }
    #[cfg(target_os = "linux")]
    fn install_desktop(&self) -> Result<(), String> {
        let directory = draugr_time_daemon::paths::default_dir().map_err(|e| e.to_string())?;
        let root = directory
            .parent()
            .ok_or("Cannot find local data directory")?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        write_desktop(root, &executable)
    }
    #[cfg(target_os = "macos")]
    fn open_permissions(&self) -> Result<(), String> {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(any(target_os = "linux", test))]
fn write_desktop(root: &Path, executable: &Path) -> Result<(), String> {
    let executable = executable
        .to_str()
        .ok_or("Executable path is not Unicode")?;
    if executable.contains(['\n', '\r']) {
        return Err("Invalid executable path".into());
    }
    let escaped = executable
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$")
        .replace('%', "%%")
        .replace('\\', "\\\\");
    let applications = root.join("applications");
    let icons = root.join("icons/hicolor/128x128/apps");
    std::fs::create_dir_all(&applications).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&icons).map_err(|e| e.to_string())?;
    std::fs::write(
        icons.join("org.draugrtime.app.png"),
        include_bytes!("../../../logo/appIcon128.png"),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(applications.join("org.draugrtime.app.desktop"),format!("[Desktop Entry]\nType=Application\nName=Draugr Time\nComment=Private local activity tracking\nExec=\"{escaped}\"\nIcon=org.draugrtime.app\nTerminal=false\nCategories=Office;\nStartupWMClass=org.draugrtime.app\n")).map_err(|e|e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_entry_uses_an_absolute_escaped_executable_and_hicolor_icon() {
        let root = tempfile::tempdir().unwrap();
        write_desktop(root.path(), Path::new("/tmp/Draugr Time 100%/draugr-time")).unwrap();
        let desktop =
            std::fs::read_to_string(root.path().join("applications/org.draugrtime.app.desktop"))
                .unwrap();
        assert!(desktop.contains("Exec=\"/tmp/Draugr Time 100%%/draugr-time\""));
        assert!(root
            .path()
            .join("icons/hicolor/128x128/apps/org.draugrtime.app.png")
            .is_file());
        assert!(write_desktop(root.path(), Path::new("/tmp/bad\npath")).is_err());
    }
    #[test]
    fn bridges_install_bundled_assets_in_desktop_discovery_locations() {
        let root = tempfile::tempdir().unwrap();
        install(root.path(), Bridge::Gnome).unwrap();
        install(root.path(), Bridge::Kde).unwrap();
        assert!(root
            .path()
            .join("gnome-shell/extensions/dagdatime@dagdatime.org/extension.js")
            .is_file());
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("kwin/scripts/dagdatime/metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["X-Plasma-MainScript"], "ui/main.qml");
        assert!(root
            .path()
            .join("kwin/scripts/dagdatime/contents/ui/main.qml")
            .is_file());
    }
}
