use crate::{now_ms, paths, Shared, Tracker};
use draugr_time_core::Storage;
use draugr_time_watchers::{IdleWatcher, NativeWatcher, WatcherError, WindowInfo, WindowWatcher};
use std::{
    error::Error,
    time::{Duration, Instant},
};

struct Unavailable(WatcherError);
impl WindowWatcher for Unavailable {
    fn active_window(&self) -> Result<Option<WindowInfo>, WatcherError> {
        Err(self.0.clone())
    }
}
impl IdleWatcher for Unavailable {
    fn idle_seconds(&self) -> Result<u64, WatcherError> {
        Err(self.0.clone())
    }
}

pub fn serve(idle_threshold: u64) -> Result<(), Box<dyn Error>> {
    let directory = paths::data_dir()?;
    std::fs::create_dir_all(&directory)?;
    let token = paths::token(&directory.join("api-token"))?;
    let storage = Storage::open(directory.join("activity.sqlite"))?;
    let shared = Shared::new(storage, token, idle_threshold);
    *shared.preferences.lock().unwrap() =
        crate::preferences::Preferences::load(&directory.join("settings.json"))?;
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, paths::PORT))?;
    listener.set_nonblocking(true)?;
    shared.begin_recording(now_ms())?;
    shared.set_pause_file(directory.join("pause.json"));
    shared.restore_pause(now_ms())?;
    let computer = start_computer(&shared);
    let server = spawn_api(shared.clone(), listener);
    let mut tracker = Tracker::new(shared.clone());
    let mut watcher = NativeWatcher::new();
    let mut last_retry = Instant::now();
    eprintln!("Draugr Time daemon listening on localhost:{}", paths::PORT);
    loop {
        if shared.control.lock().unwrap().shutdown {
            break;
        }
        if let Some(computer) = &computer {
            use draugr_time_watchers::ComputerWatcher;
            shared.control.lock().unwrap().status.computer_capabilities = computer.capabilities();
        }
        let start = Instant::now();
        if watcher.is_err() && last_retry.elapsed() >= Duration::from_secs(10) {
            watcher = NativeWatcher::new();
            last_retry = Instant::now();
        }
        let result = match &watcher {
            Ok(watcher) => tracker.poll_once(now_ms(), watcher, watcher),
            Err(error) => {
                let unavailable = Unavailable(error.clone());
                tracker.poll_once(now_ms(), &unavailable, &unavailable)
            }
        };
        if let Err(error) = result {
            let mut control = shared.control.lock().unwrap();
            control.status.state = "error".into();
            control.status.error = Some(error.to_string());
        }
        std::thread::sleep(Duration::from_secs(1).saturating_sub(start.elapsed()));
    }
    server.join().map_err(|_| "API server thread panicked")??;
    Ok(())
}

pub fn spawn_api(
    shared: Shared,
    listener: std::net::TcpListener,
) -> std::thread::JoinHandle<std::io::Result<()>> {
    let server_shared = shared.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let shutdown = server_shared.clone();
            let result = axum::serve(listener,crate::api::router(server_shared.clone())).with_graceful_shutdown(async move {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => { if let Err(error) = shutdown.shutdown() { eprintln!("Shutdown flush failed: {error}"); } }
                    _ = shutdown.wait_for_shutdown() => {}
                }
            }).await;
            if let Err(error) = server_shared.shutdown() { eprintln!("Shutdown flush failed: {error}"); }
            result
        })
    })
}

/// Keep the returned native registration alive for the recording session.
pub fn start_computer(shared: &Shared) -> Option<draugr_time_watchers::NativeComputerWatcher> {
    use draugr_time_watchers::ComputerWatcher;
    let callback_shared = shared.clone();
    let callback = std::sync::Arc::new(move |event| {
        if let Err(error) = callback_shared.lifecycle(event) {
            let mut control = callback_shared.control.lock().unwrap();
            control.status.error = Some(format!("Computer lifecycle persistence failed: {error}"));
            control.status.state = "error".into();
        }
    });
    match draugr_time_watchers::NativeComputerWatcher::start(callback) {
        Ok(watcher) => {
            shared.control.lock().unwrap().status.computer_capabilities = watcher.capabilities();
            Some(watcher)
        }
        Err(error) => {
            shared.control.lock().unwrap().status.computer_capabilities=format!("Computer lifecycle unavailable: {error}; sleep/lock/off cannot be inferred from gaps");
            None
        }
    }
}
