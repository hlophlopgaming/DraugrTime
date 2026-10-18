use auto_launch::{AutoLaunch, AutoLaunchBuilder};

pub struct LaunchOptions {
    pub record_on_launch: bool,
    pub hidden: bool,
}

impl LaunchOptions {
    pub fn parse(args: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut options = Self {
            record_on_launch: false,
            hidden: false,
        };
        for arg in args {
            match arg.as_ref() {
                "--autostart" => options.record_on_launch = true,
                "--minimized" => options.hidden = true,
                _ => {}
            }
        }
        options
    }
}

/// Prefer `$APPIMAGE` (the persistent `.AppImage` path) over `current_exe()`:
/// inside an AppImage the executable path points at the temporary squashfs mount
/// (`/tmp/.mount_…/usr/bin/…`), which is gone once the process exits, so an
/// autostart entry recorded with it never launches at login.
fn registration_path(appimage: Option<&str>, executable: Option<&str>) -> Option<String> {
    appimage
        .filter(|path| !path.is_empty())
        .or(executable)
        .map(str::to_string)
}

pub trait StartupProvider {
    fn is_enabled(&self) -> Result<bool, String>;
    fn set_enabled(&mut self, enabled: bool) -> Result<(), String>;
}

pub struct SystemStartup(AutoLaunch);

impl SystemStartup {
    pub fn new() -> Result<Self, String> {
        Self::for_name("Draugr Time", "org.draugrtime.app")
    }

    pub fn migrate_legacy(&mut self) -> Result<(), String> {
        let mut legacy = Self::for_name("DagdaTime", "org.dagdatime.app")?;
        migrate_registration(&mut legacy, self)
    }

    fn for_name(name: &str, bundle_identifier: &str) -> Result<Self, String> {
        let current_exe = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .into_owned();
        let path = registration_path(
            std::env::var("APPIMAGE").ok().as_deref(),
            Some(&current_exe),
        )
        .ok_or_else(|| "could not resolve the executable path".to_string())?;
        let mut builder = AutoLaunchBuilder::new();
        builder
            .set_app_name(name)
            .set_app_path(&path)
            .set_args(&["--autostart", "--minimized"])
            .set_bundle_identifiers(&[bundle_identifier]);
        #[cfg(target_os = "linux")]
        builder.set_linux_launch_mode(auto_launch::LinuxLaunchMode::XdgAutostart);
        #[cfg(target_os = "macos")]
        builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
        #[cfg(target_os = "windows")]
        builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
        builder.build().map(Self).map_err(|error| error.to_string())
    }
}

impl StartupProvider for SystemStartup {
    fn is_enabled(&self) -> Result<bool, String> {
        self.0.is_enabled().map_err(|error| error.to_string())
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        if enabled {
            self.0.enable()
        } else {
            self.0.disable()
        }
        .map_err(|error| error.to_string())
    }
}

pub fn set_enabled(provider: &mut impl StartupProvider, enabled: bool) -> Result<bool, String> {
    provider.set_enabled(enabled)?;
    provider.is_enabled()
}

pub fn migrate_registration(
    legacy: &mut impl StartupProvider,
    current: &mut impl StartupProvider,
) -> Result<(), String> {
    if legacy.is_enabled()? {
        if !set_enabled(current, true)? {
            return Err("New startup registration did not become enabled".into());
        }
        if set_enabled(legacy, false)? {
            return Err("Legacy startup registration could not be disabled".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockStartup {
        enabled: bool,
    }

    impl StartupProvider for MockStartup {
        fn is_enabled(&self) -> Result<bool, String> {
            Ok(self.enabled)
        }

        fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
            self.enabled = enabled;
            Ok(())
        }
    }

    #[test]
    fn rename_moves_enabled_startup_to_the_new_registration() {
        let mut legacy = MockStartup { enabled: true };
        let mut current = MockStartup::default();
        migrate_registration(&mut legacy, &mut current).unwrap();
        assert!(current.enabled);
        assert!(!legacy.enabled);
        migrate_registration(&mut legacy, &mut current).unwrap();
        assert!(current.enabled);
    }

    #[test]
    fn failed_new_startup_registration_preserves_the_legacy_entry() {
        struct FailingStartup;
        impl StartupProvider for FailingStartup {
            fn is_enabled(&self) -> Result<bool, String> {
                Ok(false)
            }
            fn set_enabled(&mut self, _: bool) -> Result<(), String> {
                Err("cannot register".into())
            }
        }
        let mut legacy = MockStartup { enabled: true };
        assert!(migrate_registration(&mut legacy, &mut FailingStartup).is_err());
        assert!(legacy.enabled);
    }

    #[test]
    fn rename_keeps_startup_disabled_when_it_was_disabled() {
        let mut legacy = MockStartup::default();
        let mut current = MockStartup::default();
        migrate_registration(&mut legacy, &mut current).unwrap();
        assert!(!legacy.enabled);
        assert!(!current.enabled);
    }

    #[test]
    fn login_setting_updates_and_reports_provider_state() {
        let mut startup = MockStartup::default();
        assert!(!startup.is_enabled().unwrap());
        assert!(set_enabled(&mut startup, true).unwrap());
        assert!(!set_enabled(&mut startup, false).unwrap());
    }

    #[test]
    fn appimage_path_wins_over_the_temporary_mount() {
        assert_eq!(
            registration_path(
                Some("/home/me/Draugr-Time.AppImage"),
                Some("/tmp/.mount_xx/usr/bin/draugr-time")
            ),
            Some("/home/me/Draugr-Time.AppImage".to_string())
        );
        assert_eq!(
            registration_path(Some(""), Some("/tmp/.mount_xx/usr/bin/draugr-time")),
            Some("/tmp/.mount_xx/usr/bin/draugr-time".to_string())
        );
        assert_eq!(
            registration_path(None, Some("/usr/bin/draugr-time")),
            Some("/usr/bin/draugr-time".to_string())
        );
        assert_eq!(registration_path(None, None), None);
    }

    #[test]
    fn autostart_launches_hidden_and_starts_recording() {
        let options = LaunchOptions::parse(["--autostart", "--minimized"]);
        assert!(options.record_on_launch);
        assert!(options.hidden);

        let manual = LaunchOptions::parse(std::iter::empty::<&str>());
        assert!(!manual.record_on_launch);
        assert!(!manual.hidden);
    }
}
