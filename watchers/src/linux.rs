use super::{IdleWatcher, WatcherError, WindowInfo, WindowWatcher};
use x11rb::{
    connection::Connection,
    protocol::{
        screensaver::ConnectionExt as _,
        xproto::{AtomEnum, ConnectionExt as _, Window},
    },
    rust_connection::RustConnection,
};

pub struct X11Watcher {
    connection: RustConnection,
    root: Window,
    active: u32,
    title: u32,
    utf8: u32,
    pid: u32,
}

pub(crate) fn check_session(
    session: Option<&str>,
    wayland_display: Option<&str>,
) -> Result<(), WatcherError> {
    if session == Some("wayland") || wayland_display.is_some_and(|display| !display.is_empty()) {
        return Err(WatcherError::Unsupported(
            "Wayland tracking arrives in M3; XWayland does not cover native windows",
        ));
    }
    Ok(())
}

fn platform(error: impl std::fmt::Display) -> WatcherError {
    WatcherError::Platform(error.to_string())
}

impl X11Watcher {
    pub fn new() -> Result<Self, WatcherError> {
        check_session(
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        )?;
        let display = std::env::var("DISPLAY").map_err(|_| {
            WatcherError::Unsupported("no X11 DISPLAY; run tracker debug in a desktop session")
        })?;
        // Only Unix-socket displays: never connect to a remote X server.
        if !display.starts_with(':') && !display.starts_with("unix:") {
            return Err(WatcherError::Unsupported(
                "only local Unix-socket X11 displays are supported",
            ));
        }
        let (connection, screen) = x11rb::connect(Some(&display)).map_err(platform)?;
        let root = connection.setup().roots[screen].root;
        let atom = |name: &[u8]| -> Result<u32, WatcherError> {
            Ok(connection
                .intern_atom(false, name)
                .map_err(platform)?
                .reply()
                .map_err(platform)?
                .atom)
        };
        let active = atom(b"_NET_ACTIVE_WINDOW")?;
        let title = atom(b"_NET_WM_NAME")?;
        let utf8 = atom(b"UTF8_STRING")?;
        let pid = atom(b"_NET_WM_PID")?;
        Ok(Self {
            connection,
            root,
            active,
            title,
            utf8,
            pid,
        })
    }

    fn property(
        &self,
        window: Window,
        atom: u32,
        kind: u32,
    ) -> Result<x11rb::protocol::xproto::GetPropertyReply, WatcherError> {
        self.connection
            .get_property(false, window, atom, kind, 0, 16_384)
            .map_err(platform)?
            .reply()
            .map_err(platform)
    }
}

impl X11Watcher {
    /// Errors for requests whose cookies were dropped unanswered (an early `?`
    /// below, e.g. BadWindow when the focused window closes between requests) are
    /// delivered to the event queue. Nothing else reads it, so drain it each sample
    /// or it grows for the life of the process.
    fn drain_events(&self) {
        while let Ok(Some(_)) = self.connection.poll_for_event() {}
    }
}

impl WindowWatcher for X11Watcher {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        self.drain_events();
        let property = self.property(self.root, self.active, AtomEnum::WINDOW.into())?;
        let Some(window) = property
            .value32()
            .and_then(|mut values| values.next())
            .filter(|window| *window != 0)
        else {
            return Ok(None);
        };
        // The active window must be known first; its independent properties share one RTT.
        let title = self
            .connection
            .get_property(false, window, self.title, self.utf8, 0, 16_384)
            .map_err(platform)?;
        let legacy = self
            .connection
            .get_property(
                false,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                0,
                16_384,
            )
            .map_err(platform)?;
        let class = self
            .connection
            .get_property(
                false,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                0,
                16_384,
            )
            .map_err(platform)?;
        let pid = self
            .connection
            .get_property(false, window, self.pid, AtomEnum::CARDINAL, 0, 1)
            .map_err(platform)?;
        let title = title.reply().map_err(platform)?;
        let legacy = legacy.reply().map_err(platform)?;
        let title = if title.type_ == self.utf8 {
            String::from_utf8_lossy(&title.value).into_owned()
        } else {
            legacy.value.iter().map(|byte| char::from(*byte)).collect()
        };
        let class = class.reply().map_err(platform)?;
        let parts: Vec<_> = class
            .value
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .collect();
        let app = parts
            .last()
            .map(|value| String::from_utf8_lossy(value).into_owned())
            .unwrap_or_default();
        let pid = pid
            .reply()
            .map_err(platform)?
            .value32()
            .and_then(|mut values| values.next())
            .filter(|pid| *pid != 0);
        Ok(Some(WindowInfo {
            app_name: app.clone(),
            app,
            title,
            pid,
        }))
    }
}

impl IdleWatcher for X11Watcher {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        let reply = self
            .connection
            .screensaver_query_info(self.root)
            .map_err(platform)?
            .reply()
            .map_err(platform)?;
        Ok(u64::from(reply.ms_since_user_input) / 1_000)
    }
}
