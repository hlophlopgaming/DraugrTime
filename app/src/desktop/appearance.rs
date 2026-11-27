#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Preference {
	System,
	Light,
	Dark,
}

impl Preference {
	pub(super) fn is_dark(self, system_mode: iced::theme::Mode) -> bool {
		match self {
			Self::System => system_mode == iced::theme::Mode::Dark,
			Self::Light => false,
			Self::Dark => true,
		}
	}

	pub(super) fn next(self) -> Self {
		match self {
			Self::System => Self::Light,
			Self::Light => Self::Dark,
			Self::Dark => Self::System,
		}
	}

	pub(super) fn label(self) -> &'static str {
		match self {
			Self::System => "System",
			Self::Light => "Light",
			Self::Dark => "Dark",
		}
	}
}

#[cfg(target_os = "linux")]
pub(super) fn system_mode() -> Option<iced::theme::Mode> {
	use zbus::blocking::{Connection, Proxy};

	let connection = Connection::session().ok()?;
	let proxy = Proxy::new(
		&connection,
		"org.freedesktop.portal.Desktop",
		"/org/freedesktop/portal/desktop",
		"org.freedesktop.portal.Settings",
	)
	.ok()?;
	let scheme: zbus::zvariant::OwnedValue = proxy
		.call("ReadOne", &("org.freedesktop.appearance", "color-scheme"))
		.ok()?;
	let scheme = u32::try_from(scheme).ok()?;
	Some(portal_color_scheme(scheme))
}

#[cfg(target_os = "linux")]
const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
#[cfg(target_os = "linux")]
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
#[cfg(target_os = "linux")]
const PORTAL_SETTINGS: &str = "org.freedesktop.portal.Settings";
#[cfg(target_os = "linux")]
const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";
#[cfg(target_os = "linux")]
const COLOR_SCHEME_KEY: &str = "color-scheme";

/// Emits the current color scheme, then every change the XDG portal announces
/// through `SettingChanged`. No polling: the GUI only wakes when the desktop
/// actually switches theme. Ends quietly when there is no session bus or portal.
#[cfg(target_os = "linux")]
pub(super) fn portal_theme_changes() -> impl iced::futures::Stream<Item = iced::theme::Mode> {
	use iced::futures::{SinkExt, StreamExt};

	iced::stream::channel(4, async move |mut output| {
		let Ok(connection) = zbus::Connection::session().await else {
			return;
		};
		let Ok(proxy) = zbus::Proxy::new(
			&connection,
			PORTAL_DESTINATION,
			PORTAL_PATH,
			PORTAL_SETTINGS,
		)
		.await
		else {
			return;
		};
		// Subscribe before reading so a change between the two is not missed.
		let Ok(mut changes) = proxy.receive_signal("SettingChanged").await else {
			return;
		};
		let current: zbus::Result<zbus::zvariant::OwnedValue> = proxy
			.call("ReadOne", &(APPEARANCE_NAMESPACE, COLOR_SCHEME_KEY))
			.await;
		if let Some(mode) = current
			.ok()
			.and_then(|value| u32::try_from(value).ok())
			.map(portal_color_scheme)
		{
			if output.send(mode).await.is_err() {
				return;
			}
		}
		while let Some(message) = changes.next().await {
			let Ok((namespace, key, value)) =
				message
					.body()
					.deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
			else {
				continue;
			};
			let scheme = u32::try_from(value).ok();
			if let Some(mode) = appearance_change(&namespace, &key, scheme) {
				if output.send(mode).await.is_err() {
					return;
				}
			}
		}
	})
}

/// Maps a portal `SettingChanged` signal to a theme mode, ignoring other settings.
#[cfg(target_os = "linux")]
fn appearance_change(namespace: &str, key: &str, scheme: Option<u32>) -> Option<iced::theme::Mode> {
	if namespace != APPEARANCE_NAMESPACE || key != COLOR_SCHEME_KEY {
		return None;
	}
	scheme.map(portal_color_scheme)
}

#[cfg(target_os = "linux")]
fn portal_color_scheme(scheme: u32) -> iced::theme::Mode {
	match scheme {
		1 => iced::theme::Mode::Dark,
		2 => iced::theme::Mode::Light,
		_ => iced::theme::Mode::None,
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn system_preference_follows_native_theme_with_light_fallback() {
		use iced::theme::Mode;

		assert!(!super::Preference::System.is_dark(Mode::Light));
		assert!(super::Preference::System.is_dark(Mode::Dark));
		assert!(!super::Preference::System.is_dark(Mode::None));
	}

	#[test]
	fn manual_light_and_dark_override_the_system_theme() {
		use iced::theme::Mode;

		assert!(!super::Preference::Light.is_dark(Mode::Dark));
		assert!(super::Preference::Dark.is_dark(Mode::Light));
	}

	#[test]
	fn theme_button_cycles_system_light_dark() {
		assert_eq!(super::Preference::System.next(), super::Preference::Light);
		assert_eq!(super::Preference::Light.next(), super::Preference::Dark);
		assert_eq!(super::Preference::Dark.next(), super::Preference::System);
	}

	#[cfg(target_os = "linux")]
	#[test]
	fn portal_color_scheme_maps_dark_and_light_preferences() {
		use iced::theme::Mode;

		assert_eq!(super::portal_color_scheme(1), Mode::Dark);
		assert_eq!(super::portal_color_scheme(2), Mode::Light);
		assert_eq!(super::portal_color_scheme(0), Mode::None);
		assert_eq!(super::portal_color_scheme(99), Mode::None);
	}

	#[cfg(target_os = "linux")]
	#[test]
	fn setting_changed_signal_maps_only_the_color_scheme() {
		use super::appearance_change;
		use iced::theme::Mode;

		let ns = "org.freedesktop.appearance";
		assert_eq!(
			appearance_change(ns, "color-scheme", Some(1)),
			Some(Mode::Dark)
		);
		assert_eq!(
			appearance_change(ns, "color-scheme", Some(2)),
			Some(Mode::Light)
		);
		assert_eq!(
			appearance_change(ns, "color-scheme", Some(0)),
			Some(Mode::None)
		);
		assert_eq!(appearance_change(ns, "accent-color", Some(1)), None);
		assert_eq!(
			appearance_change("org.gnome.desktop.interface", "color-scheme", Some(1)),
			None
		);
		assert_eq!(
			appearance_change(ns, "color-scheme", None),
			None,
			"non-integer values are ignored"
		);
	}

	#[cfg(target_os = "linux")]
	#[test]
	#[ignore = "requires a desktop session portal with an explicit color preference"]
	fn portal_stream_reports_the_current_scheme_first() {
		use iced::futures::StreamExt;

		let mut changes = std::pin::pin!(super::portal_theme_changes());
		let first = tokio::runtime::Builder::new_current_thread()
			.build()
			.unwrap()
			.block_on(changes.next());
		assert_ne!(first, Some(iced::theme::Mode::None));
		assert!(first.is_some());
	}

	#[cfg(target_os = "linux")]
	#[test]
	#[ignore = "requires a desktop session portal with an explicit color preference"]
	fn reads_system_theme_from_the_desktop_portal() {
		assert_ne!(super::system_mode(), Some(iced::theme::Mode::None));
	}
}
