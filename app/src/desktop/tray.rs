use super::{
    localization::{self, tr},
    model,
};
use draugr_time_daemon::Status;
use std::cell::RefCell;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct NativeTray {
    icon: TrayIcon,
    status: MenuItem,
    activity: MenuItem,
    total: MenuItem,
    pauses: Vec<CheckMenuItem>,
    pause_menu: Submenu,
    resume: MenuItem,
    extensions: Vec<MenuItem>,
    /// What the tray currently shows. `update` runs every second; menu text and the
    /// icon are only pushed to the OS when they change (on Linux every `set_icon`
    /// writes a new PNG for the indicator).
    shown: RefCell<Option<Shown>>,
    labels: Vec<(MenuItem, &'static str)>,
}
#[derive(Clone, PartialEq)]
struct Shown {
    label: String,
    language: localization::Language,
    color: [u8; 3],
    activity: String,
    total: String,
    pause: bool,
    resume: bool,
    last_pause: i64,
}
pub fn pause_duration(id: &str) -> Option<i64> {
    match id {
        "pause5" => Some(300_000),
        "pause10" => Some(600_000),
        "pause15" => Some(900_000),
        "pause30" => Some(1_800_000),
        "pause60" => Some(3_600_000),
        _ => None,
    }
}
static LAST_PAUSE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(15);

pub fn remember_pause(duration: Option<i64>) {
    if let Some(duration) =
        duration.filter(|value| [300_000, 600_000, 900_000, 1_800_000, 3_600_000].contains(value))
    {
        LAST_PAUSE.store(duration / 60_000, std::sync::atomic::Ordering::Relaxed);
    }
}

fn pause_label(minutes: i64) -> String {
    super::localized("{count} min", &[("count", minutes.to_string())])
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    #[test]
    fn tray_has_only_the_five_timed_pause_presets() {
        for minutes in [5, 10, 15, 30, 60] {
            assert_eq!(
                pause_duration(&format!("pause{minutes}")),
                Some(minutes * 60_000)
            );
        }
        for id in ["pause", "pause0", "pause999", "resume"] {
            assert_eq!(pause_duration(id), None);
        }
    }
}

impl NativeTray {
    pub fn new() -> Result<Self, String> {
        channel();
        let menu = Menu::new();
        let mut labels = Vec::new();
        let mut item = |id: &str, label: &'static str, enabled: bool| {
            let menu_item = MenuItem::with_id(id, tr(label), enabled, None);
            labels.push((menu_item.clone(), label));
            menu_item
        };
        let open = item("open", "Open Draugr Time", true);
        let start = item("start", "Start recording", true);
        let status = MenuItem::new(tr("Not recording"), false, None);
        let activity = MenuItem::new(tr("No current activity"), false, None);
        let total = MenuItem::new(tr("Active time: —"), false, None);
        let quick = item("quick15", "Tag last 15 minutes", true);
        let quick5 = item("quick5", "Tag last 5 minutes", true);
        let quick60 = item("quick60", "Tag last hour", true);
        let pause_menu = Submenu::new(tr("I'm away · don't track for"), true);
        let mut pauses = Vec::new();
        for minutes in [5, 10, 15, 30, 60] {
            let pause = CheckMenuItem::with_id(
                format!("pause{minutes}"),
                pause_label(minutes),
                false,
                minutes == 15,
                None,
            );
            pause_menu.append(&pause).map_err(|e| e.to_string())?;
            pauses.push(pause);
        }
        let hint = item("pause_hint", "Turns back on by itself", false);
        pause_menu.append(&hint).map_err(|e| e.to_string())?;
        let resume = item("resume", "Resume tracking", false);
        let extensions: Vec<_> = [5, 15]
            .into_iter()
            .map(|minutes| {
                MenuItem::with_id(
                    format!("extend{minutes}"),
                    format!("+{}", pause_label(minutes)),
                    false,
                    None,
                )
            })
            .collect();
        let settings = item("settings", "Settings", true);
        let quit = item("quit", "Quit Draugr Time", true);
        menu.append_items(&[
            &status,
            &activity,
            &total,
            &PredefinedMenuItem::separator(),
            &open,
            &start,
            &quick5,
            &quick,
            &quick60,
            &pause_menu,
            &resume,
            &extensions[0],
            &extensions[1],
            &PredefinedMenuItem::separator(),
            &settings,
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let icon = TrayIconBuilder::new()
            .with_tooltip(tr("Draugr Time · Not recording"))
            .with_icon(Self::image(crate::branding::TRAY_NOT_RECORDING))
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            icon,
            status,
            activity,
            total,
            pauses,
            pause_menu,
            resume,
            extensions,
            shown: RefCell::new(None),
            labels,
        })
    }
    fn image(color: [u8; 3]) -> Icon {
        let rgba = crate::branding::tray_rgba(color);
        Icon::from_rgba(rgba, 32, 32).expect("valid tray image")
    }
    pub fn update(&self, status: &Status, active: i64, fixture: bool) {
        let state = status.state.as_str();
        let (label, color) = match state {
            // One colour per meaning: brand mint while recording (including
            // AFK/locked/asleep, which are recorded states), red whenever
            // nothing is being recorded. See docs/design.md, "Tray icon".
            "tracking" => ("Tracking", crate::branding::TRAY_RECORDING),
            "away" => ("AFK", crate::branding::TRAY_RECORDING),
            "locked" => ("Locked", crate::branding::TRAY_RECORDING),
            "asleep" => ("Sleep", crate::branding::TRAY_RECORDING),
            "paused" => ("Paused", crate::branding::TRAY_NOT_RECORDING),
            "error" => ("Needs attention", crate::branding::TRAY_NOT_RECORDING),
            "starting" => ("Starting", [130, 145, 160]),
            _ => ("Not recording", crate::branding::TRAY_NOT_RECORDING),
        };
        let next = Shown {
            label: if let Some(until) = status.paused_until.filter(|_| state == "paused") {
                format!(
                    "{} · {}",
                    tr(label),
                    model::duration(until.saturating_sub(draugr_time_daemon::now_ms()).max(0))
                )
            } else {
                tr(label)
            },
            language: localization::current(),
            color,
            activity: status
                .current_window
                .as_ref()
                .map(|window| model::process_label(&window.app))
                .map(str::to_owned)
                .unwrap_or_else(|| tr("No current activity")),
            total: format!(
                "{}: {}",
                tr(if fixture {
                    "Fixture active time"
                } else {
                    "Selected day active time"
                }),
                model::duration(active)
            ),
            pause: !state.is_empty() && state != "preview" && state != "paused",
            resume: state == "paused",
            last_pause: LAST_PAUSE.load(std::sync::atomic::Ordering::Relaxed),
        };
        let mut shown = self.shown.borrow_mut();
        let old = shown.as_ref();
        if old.is_none_or(|old| old.language != next.language) {
            self.pause_menu.set_text(tr("I'm away · don't track for"));
            for (item, minutes) in self.pauses.iter().zip([5, 10, 15, 30, 60]) {
                item.set_text(pause_label(minutes));
            }
            for (item, minutes) in self.extensions.iter().zip([5, 15]) {
                item.set_text(format!("+{}", pause_label(minutes)));
            }
            for (item, key) in &self.labels {
                item.set_text(tr(key));
            }
        }
        if old.is_none_or(|old| old.label != next.label) {
            self.status.set_text(&next.label);
        }
        if old.is_none_or(|old| old.activity != next.activity) {
            self.activity.set_text(&next.activity);
        }
        if old.is_none_or(|old| old.total != next.total) {
            self.total.set_text(&next.total);
        }
        for (item, minutes) in self.pauses.iter().zip([5, 10, 15, 30, 60]) {
            let checked = minutes == next.last_pause;
            if item.is_checked() != checked {
                item.set_checked(checked);
            }
        }
        if old.is_none_or(|old| (old.pause, old.resume) != (next.pause, next.resume)) {
            for item in &self.pauses {
                item.set_enabled(next.pause || next.resume);
            }
        }
        if old.is_none_or(|old| old.resume != next.resume) {
            self.resume.set_enabled(next.resume);
            for item in &self.extensions {
                item.set_enabled(next.resume);
            }
        }
        if old.is_none_or(|old| old.color != next.color) {
            let _ = self.icon.set_icon(Some(Self::image(next.color)));
        }
        if old.is_none_or(|old| old.label != next.label) {
            let _ = self
                .icon
                .set_tooltip(Some(format!("Draugr Time · {}", next.label)));
        }
        *shown = Some(next);
    }
}

#[cfg(not(target_os = "linux"))]
pub type Tray = NativeTray;
#[derive(Clone, Debug)]
pub enum Event {
    Menu(String),
    Status(std::sync::Arc<Status>),
}
fn channel() -> &'static tokio::sync::broadcast::Sender<Event> {
    static CHANNEL: std::sync::OnceLock<tokio::sync::broadcast::Sender<Event>> =
        std::sync::OnceLock::new();
    CHANNEL.get_or_init(|| {
        let (sender, _) = tokio::sync::broadcast::channel(64);
        let events = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = events.send(Event::Menu(event.id.as_ref().to_owned()));
        }));
        sender
    })
}
pub fn push_status(status: Status) {
    let _ = channel().send(Event::Status(std::sync::Arc::new(status)));
}
pub fn events() -> impl iced::futures::Stream<Item = Event> {
    use iced::futures::SinkExt;
    let mut receiver = channel().subscribe();
    iced::stream::channel(64, async move |mut output| loop {
        match receiver.recv().await {
            Ok(event) => {
                if output.send(event).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    })
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::sync::mpsc;

    enum Command {
        Update(Box<Status>, i64, bool),
        Quit,
    }
    trait Backend {
        fn update(&self, status: &Status, active: i64, fixture: bool);
    }
    impl Backend for NativeTray {
        fn update(&self, status: &Status, active: i64, fixture: bool) {
            NativeTray::update(self, status, active, fixture);
        }
    }
    pub struct Tray {
        sender: mpsc::Sender<Command>,
    }
    impl Tray {
        pub fn new() -> Result<Self, String> {
            spawn_service(NativeTray::new)
        }
        pub fn update(&self, status: &Status, active: i64, fixture: bool) {
            let _ = self
                .sender
                .send(Command::Update(Box::new(status.clone()), active, fixture));
        }
    }
    impl Drop for Tray {
        fn drop(&mut self) {
            let _ = self.sender.send(Command::Quit);
        }
    }
    fn spawn_service<B: Backend + 'static>(
        init: impl FnOnce() -> Result<B, String> + Send + 'static,
    ) -> Result<Tray, String> {
        let (sender, receiver) = mpsc::channel();
        let (ready, started) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let backend = match init() {
                Ok(backend) => backend,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            if ready.send(Ok(())).is_err() {
                return;
            }
            while let Ok(mut command) = receiver.recv() {
                // Apply only the newest queued state: if the panel stalled (e.g. it
                // restarts after resume), replaying every queued Status is wasted work.
                while !matches!(command, Command::Quit) {
                    match receiver.try_recv() {
                        Ok(next) => command = next,
                        Err(_) => break,
                    }
                }
                match command {
                    Command::Update(status, active, fixture) => {
                        backend.update(&status, active, fixture)
                    }
                    Command::Quit => break,
                }
            }
        });
        started.recv().map_err(|error| error.to_string())??;
        Ok(Tray { sender })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::Duration;
        struct MockBackend(mpsc::Sender<Status>);
        impl Backend for MockBackend {
            fn update(&self, status: &Status, _: i64, _: bool) {
                self.0.send(status.clone()).unwrap();
            }
        }
        #[test]
        #[ignore = "requires a private D-Bus session (dbus-run-session)"]
        fn native_ksni_menu_dispatches_without_gui_ticks() {
            let host = lifecycle_host().unwrap();
            assert!(
                host.is_some(),
                "run on a private bus, not the user's desktop"
            );
            let mut receiver = channel().subscribe();
            let tray = Tray::new().unwrap();
            let connection = zbus::blocking::Connection::session().unwrap();
            let dbus = zbus::blocking::fdo::DBusProxy::new(&connection).unwrap();
            let name = dbus
                .list_names()
                .unwrap()
                .into_iter()
                .find(|name| {
                    name.as_str().starts_with(&format!(
                        "org.kde.StatusNotifierItem-{}-",
                        std::process::id()
                    ))
                })
                .unwrap();
            let proxy = zbus::blocking::Proxy::new(
                &connection,
                name.as_str(),
                "/MenuBar",
                "com.canonical.dbusmenu",
            )
            .unwrap();
            let _: () = proxy
                .call(
                    "Event",
                    &(5_i32, "clicked", zbus::zvariant::Value::from(0_i32), 0_u32),
                )
                .unwrap();
            let event = receiver.try_recv().unwrap();
            assert!(matches!(event, Event::Menu(id) if id == "open"));
            drop(tray);
        }
        #[test]
        fn tray_services_native_events_without_gui_ticks() {
            let (event, received) = mpsc::channel();
            let (updates, updated) = mpsc::channel();
            let tray = spawn_service(move || {
                event.send(()).unwrap();
                Ok(MockBackend(updates))
            })
            .unwrap();
            let result = received.recv_timeout(Duration::from_secs(1));
            let status = Status {
                state: "paused".into(),
                ..Default::default()
            };
            tray.update(&status, 0, false);
            let update = updated.recv_timeout(Duration::from_secs(1));
            drop(tray);
            assert_eq!(update.unwrap(), status);
            assert!(
                result.is_ok(),
                "tray must service native events without GUI ticks"
            );
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::Tray;

#[cfg(target_os = "linux")]
#[derive(Default)]
struct FixtureWatcher;
#[cfg(target_os = "linux")]
#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl FixtureWatcher {
    fn register_status_notifier_item(&self, _service: &str) {}
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
}
/// CI has a private session bus but no desktop panel; this host is only for lifecycle diagnostics.
#[cfg(target_os = "linux")]
pub fn lifecycle_host() -> Result<Option<zbus::blocking::Connection>, String> {
    let connection = zbus::blocking::Connection::session().map_err(|error| error.to_string())?;
    let dbus =
        zbus::blocking::fdo::DBusProxy::new(&connection).map_err(|error| error.to_string())?;
    if dbus
        .name_has_owner("org.kde.StatusNotifierWatcher".try_into().unwrap())
        .map_err(|error| error.to_string())?
    {
        return Ok(None);
    }
    connection
        .object_server()
        .at("/StatusNotifierWatcher", FixtureWatcher)
        .map_err(|error| error.to_string())?;
    connection
        .request_name("org.kde.StatusNotifierWatcher")
        .map_err(|error| error.to_string())?;
    eprintln!("Lifecycle diagnostic: using fixture StatusNotifierWatcher (no real panel)");
    Ok(Some(connection))
}
