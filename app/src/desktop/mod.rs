//! Native Draugr Time desktop.
mod appearance;
mod localization;
mod reports;
#[cfg(test)]
mod reports_tests;
mod rules;
mod settings;
#[cfg(test)]
mod settings_tests;
mod setup;
use localization::{message as localized, tr, LocaleProvider};
mod memory;
mod model;
mod runtime;
mod single_instance;
mod startup;
mod tagging;
mod ticking;
mod timeline;
mod tray;

use chrono::{Days, Local, NaiveDate};
use iced::{
    keyboard,
    widget::{
        self, button, checkbox, column, container, image, pane_grid, row, scrollable, slider,
        space, svg, text, text_input, tooltip,
    },
    window, Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use memory::MemoryReclaimer;
use model::{Range, Summary, Track};
use runtime::{Runtime, Snapshot};
use startup::StartupProvider;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

static TEST_PASSED: AtomicBool = AtomicBool::new(false);

/// OS command used to open a URL in the user's default browser.
fn open_command(url: &str) -> (&'static str, Vec<String>) {
    #[cfg(target_os = "windows")]
    return (
        "cmd",
        vec!["/C".into(), "start".into(), String::new(), url.into()],
    );
    #[cfg(target_os = "macos")]
    return ("open", vec![url.into()]);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return ("xdg-open", vec![url.into()]);
}

fn open_url(url: &str) {
    let (program, args) = open_command(url);
    if let Err(error) = std::process::Command::new(program).args(args).spawn() {
        eprintln!("failed to open {url}: {error}");
    }
}

fn cached_app_icon<'a, T>(icons: &'a HashMap<String, T>, app: &str, name: &str) -> Option<&'a T> {
    icons
        .get(&model::application_identity(app, name))
        .or_else(|| {
            if app != name {
                return None;
            }
            let mut matches = icons.iter().filter(|(key, _)| {
                key.split_once('\0')
                    .is_some_and(|(_, candidate)| candidate == name)
            });
            let (_, icon) = matches.next()?;
            matches.next().is_none().then_some(icon)
        })
}

fn decode_icons(
    icons: HashMap<String, Option<crate::icons::IconData>>,
) -> HashMap<String, AppIcon> {
    icons
        .into_iter()
        .filter_map(|(app, icon)| {
            let handle = match icon? {
                crate::icons::IconData::Png(bytes) => {
                    AppIcon::Png(image::Handle::from_bytes(bytes))
                }
                crate::icons::IconData::Svg(bytes) => AppIcon::Svg(svg::Handle::from_memory(bytes)),
            };
            Some((app, handle))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub enum Message {
    Settings(settings::Edit),
    Edit(tagging::Edit),
    Rule(rules::Edit),
    Report(reports::Edit),
    Language(Option<localization::Language>),
    Tick,
    Tray(tray::Event),
    Opened(window::Id),
    Close(window::Id),
    Closed,
    Screen(&'static str),
    Date(String),
    Shift(i32),
    Today,
    Search(String),
    Tab(usize),
    ResizeDay(pane_grid::ResizeEvent),
    Select(Option<Range>),
    Hover(Option<(usize, usize)>),
    Highlight(String),
    Zoom(f64, Option<i64>),
    Fit,
    Pan(f64),
    PanBy(f64),
    Tracks,
    Visible(usize, bool),
    Move(usize, i32),
    More,
    Theme,
    #[cfg(not(target_os = "linux"))]
    SystemThemeChanged(iced::theme::Mode),
    #[cfg(target_os = "linux")]
    PortalThemeChanged(Option<iced::theme::Mode>),
    Start,
    AutoStart(bool),
    Pause(Option<i64>),
    Resume,
    Quit,
    Folder,
    FolderChosen(Option<std::path::PathBuf>),
    OpenUrl(&'static str),
    Key(keyboard::Key, keyboard::Modifiers),
    Loaded(u64, Result<Snapshot, String>),
    Screenshot(window::Screenshot),
}
#[derive(Clone)]
pub(super) enum AppIcon {
    Png(image::Handle),
    Svg(svg::Handle),
}
#[derive(Clone, Copy, Debug)]
enum DayPane {
    Timeline,
    Summary,
}
struct State {
    settings_ui: settings::Ui,
    report_ui: reports::Ui,
    rule_editor: rules::Editor,
    language_choice: Option<localization::Language>,
    tags: Vec<draugr_time_core::Tag>,
    tag_ranges: Vec<draugr_time_core::TagRange>,
    recent_tags: Vec<i64>,
    editor: Option<tagging::Editor>,
    tag_undo: Vec<draugr_time_core::TagUndo>,
    away_prompt: tagging::AwayPrompt,
    away_range: Option<Range>,
    date: NaiveDate,
    date_text: String,
    day: Range,
    view: Range,
    all: Vec<Track>,
    filtered: Vec<Vec<usize>>,
    search_text: HashMap<usize, (Arc<str>, String)>,
    order: Vec<usize>,
    visible: Vec<usize>,
    shown: [bool; 4],
    selection: Option<Range>,
    hover: Option<(usize, usize)>,
    highlight: Option<String>,
    search: String,
    screen: &'static str,
    tab: usize,
    rows: Vec<Summary>,
    documents: Vec<Summary>,
    limit: usize,
    metrics: [i64; 4],
    fixture: bool,
    tracks_menu: bool,
    day_panes: pane_grid::State<DayPane>,
    theme: Theme,
    theme_preference: appearance::Preference,
    system_theme: iced::theme::Mode,
    grid_cache: widget::canvas::Cache,
    cache: widget::canvas::Cache,
    stats: Arc<timeline::PaintStats>,
    icons: HashMap<String, AppIcon>,
    preview_apps: Vec<(String, String)>,
    runtime: Option<Runtime>,
    tray: Option<tray::Tray>,
    status: draugr_time_daemon::Status,
    error: String,
    loading: bool,
    /// Bumped whenever the shown day or data source changes; older loads are dropped.
    load_generation: u64,
    load_cursor: runtime::LoadCursor,
    /// Day whose recorded history is currently shown; past days are loaded once.
    loaded_day: Option<Range>,
    clock: Instant,
    last_sample: Instant,
    last_load: Instant,
    window: Option<window::Id>,
    smoke: bool,
    runtime_test: bool,
    onboarding_test: bool,
    lifecycle: bool,
    cycle_stage: u8,
    cycle_duration: i64,
    cycle_frames: usize,
    benchmark: bool,
    snapshot_done: bool,
    closed_for_benchmark: bool,
    pending_dir: Option<String>,
    autostart_enabled: bool,
    autostart_error: String,
}
impl State {
    fn boot() -> (Self, Task<Message>) {
        let args: Vec<_> = std::env::args().collect();
        let launch = startup::LaunchOptions::parse(args.iter().map(String::as_str));
        let smoke = args.iter().any(|arg| arg == "--smoke-test");
        let lifecycle = args.iter().any(|arg| arg == "--lifecycle-test");
        let onboarding_test = args.iter().any(|arg| arg == "--onboarding-test");
        let runtime_test =
            onboarding_test || lifecycle || args.iter().any(|arg| arg == "--runtime-test");
        let benchmark = args.iter().any(|arg| arg == "--benchmark");
        let date = Local::now().date_naive();
        let day = model::day_bounds(date);
        let stats = Arc::new(timeline::PaintStats::default());
        let preview_apps = if cfg!(test) || !(smoke || benchmark) {
            Vec::new()
        } else {
            crate::icons::preview_application_processes()
        };
        let icons = decode_icons(crate::icons::application_icons(preview_apps.clone()));
        let language_choice = localization::path()
            .and_then(|p| localization::load(&p))
            .unwrap_or(None);
        localization::set(localization::resolve(
            language_choice,
            &localization::SystemLocale.locale(),
        ));
        let tray = if cfg!(test) || smoke || (runtime_test && !lifecycle) {
            None
        } else {
            match tray::Tray::new() {
                Ok(tray) => Some(tray),
                Err(error) => {
                    eprintln!("Tray unavailable: {error}");
                    None
                }
            }
        };
        let mut state = Self {
            settings_ui: settings::Ui::load(),
            report_ui: reports::Ui::default(),
            rule_editor: rules::Editor::default(),
            language_choice,
            tags: Vec::new(),
            tag_ranges: Vec::new(),
            recent_tags: Vec::new(),
            editor: None,
            tag_undo: Vec::new(),
            away_prompt: tagging::AwayPrompt {
                threshold: tagging::load_threshold(),
                seen_end: draugr_time_daemon::now_ms(),
            },
            away_range: None,
            date,
            date_text: date.to_string(),
            day,
            view: day,
            all: if smoke || benchmark {
                model::fixture_with_apps(day, &preview_apps)
            } else {
                model::empty_tracks()
            },
            filtered: Vec::new(),
            search_text: HashMap::new(),
            order: vec![0, 1, 2, 3],
            visible: vec![0, 1, 2, 3],
            shown: [true; 4],
            selection: None,
            hover: None,
            highlight: None,
            documents: Vec::new(),
            search: String::new(),
            screen: "Day",
            tab: 1,
            rows: Vec::new(),
            limit: 50,
            metrics: [0; 4],
            fixture: smoke || benchmark,
            tracks_menu: false,
            day_panes: pane_grid::State::with_configuration(pane_grid::Configuration::Split {
                axis: pane_grid::Axis::Horizontal,
                ratio: 0.6,
                a: Box::new(pane_grid::Configuration::Pane(DayPane::Timeline)),
                b: Box::new(pane_grid::Configuration::Pane(DayPane::Summary)),
            }),
            theme: native_theme(false),
            theme_preference: appearance::Preference::System,
            system_theme: iced::theme::Mode::None,
            grid_cache: widget::canvas::Cache::new(),
            cache: widget::canvas::Cache::new(),
            stats,
            icons,
            preview_apps,
            runtime: None,
            tray,
            status: draugr_time_daemon::Status::default(),
            error: String::new(),
            loading: false,
            load_generation: 0,
            load_cursor: runtime::LoadCursor::default(),
            loaded_day: None,
            clock: Instant::now(),
            last_sample: Instant::now() - Duration::from_secs(2),
            last_load: Instant::now() - Duration::from_secs(5),
            window: None,
            smoke,
            runtime_test,
            onboarding_test,
            lifecycle,
            cycle_stage: 0,
            cycle_duration: 0,
            cycle_frames: 0,
            benchmark,
            snapshot_done: false,
            closed_for_benchmark: false,
            pending_dir: None,
            autostart_enabled: startup::SystemStartup::new()
                .and_then(|startup| startup.is_enabled())
                .unwrap_or(false),
            autostart_error: String::new(),
        };
        if (runtime_test && !onboarding_test)
            || launch.record_on_launch
            || (!cfg!(test) && !smoke && !benchmark && state.settings_ui.preferences.onboarded)
        {
            state.start();
        }
        state.rebuild();
        let task = if launch.hidden && state.tray.is_some() {
            Task::none()
        } else {
            state.open(true)
        };
        #[cfg(target_os = "linux")]
        let system_theme = Task::perform(
            async {
                tokio::task::spawn_blocking(appearance::system_mode)
                    .await
                    .ok()
                    .flatten()
            },
            Message::PortalThemeChanged,
        );
        #[cfg(not(target_os = "linux"))]
        let system_theme = iced::system::theme().map(Message::SystemThemeChanged);
        if let Some(screen) = args
            .windows(2)
            .find(|p| p[0] == "--screen")
            .map(|p| p[1].as_str())
        {
            state.screen = match screen {
                "Overview" => "Overview",
                "Week" => "Week",
                "Reports" => "Reports",
                _ => "Day",
            };
            let _ = state.edit_report(reports::Edit::Refresh);
        }
        (state, Task::batch([task, system_theme]))
    }
    fn set_screen(&mut self, screen: &'static str) {
        if matches!(self.screen, "Week" | "Reports") && self.screen != screen {
            self.release_reports();
        }
        self.screen = screen;
    }
    fn apply_theme(&mut self) {
        self.theme = native_theme(self.theme_preference.is_dark(self.system_theme));
        self.cache.clear();
        self.grid_cache.clear();
    }
    fn open(&mut self, visible: bool) -> Task<Message> {
        if let Some(id) = self.window {
            return window::gain_focus(id);
        }
        if self.fixture && self.all.iter().all(|track| track.segments.is_empty()) {
            self.all = model::fixture_with_apps(self.day, &self.preview_apps);
            self.rebuild();
        }
        let (id, task) = window::open(window::Settings {
            size: iced::Size::new(1280.0, 820.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            visible,
            exit_on_close_request: false,
            icon: crate::branding::window_icon().ok(),
            ..window::Settings::default()
        });
        self.window = Some(id);
        task.map(Message::Opened)
    }
    fn start(&mut self) {
        if self.runtime.is_some() {
            self.fixture = false;
            self.all = model::empty_tracks();
            self.invalidate_load();
            self.rebuild();
            return;
        }
        match Runtime::start(self.runtime_test) {
            Ok(runtime) => {
                self.runtime = Some(runtime);
                self.fixture = false;
                self.all = model::empty_tracks();
                self.error.clear();
                self.invalidate_load();
            }
            Err(error) => {
                self.error = error;
            }
        }
        self.rebuild();
    }
    /// Request a fresh load and discard any load still in flight: its result belongs
    /// to a different day or data source and must not replace what is shown.
    fn invalidate_load(&mut self) {
        self.load_generation = self.load_generation.wrapping_add(1);
        self.loading = false;
        self.loaded_day = None;
        self.load_cursor = runtime::LoadCursor::default();
        self.search_text.clear();
        self.last_load = Instant::now() - Duration::from_secs(5);
    }
    fn release_day(&mut self) {
        self.report_ui.data = None;
        self.report_ui.day = draugr_time_core::Report::default();
        self.report_ui.loading = false;
        self.report_ui.generation = self.report_ui.generation.wrapping_add(1);
        self.invalidate_load();
        self.all = model::empty_tracks();
        self.filtered = vec![Vec::new(); 4];
        self.search_text = HashMap::new();
        self.rows = Vec::new();
        self.documents = Vec::new();
        self.icons = HashMap::new();
        self.tags = Vec::new();
        self.tag_ranges = Vec::new();
        self.recent_tags = Vec::new();
        self.preview_apps = Vec::new();
        self.hover = None;
        self.cache.clear();
        self.grid_cache.clear();
    }
    fn rebuild(&mut self) {
        self.report_ui.day =
            reports::from_tracks(&self.all, &self.tags, self.day).unwrap_or_default();
        let ranges = |track: usize, key: Option<&str>| {
            self.all[track]
                .segments
                .iter()
                .filter(|s| key.is_none_or(|key| s.key.as_ref() == key))
                .map(|s| s.range)
                .collect::<Vec<_>>()
        };
        self.metrics = [
            model::union_duration(&ranges(1, Some("active")), self.day),
            model::union_duration(&ranges(1, Some("away")), self.day),
            model::union_duration(
                &self.all[0]
                    .segments
                    .iter()
                    .filter(|s| s.key.as_ref() != "untagged")
                    .map(|s| s.range)
                    .collect::<Vec<_>>(),
                self.day,
            ),
            model::union_duration(&ranges(0, Some("untagged")), self.day),
        ];
        let search = self.search.to_lowercase();
        let mut previous = std::mem::take(&mut self.search_text);
        if !search.trim().is_empty() {
            for segment in self.all.iter().flat_map(|track| &track.segments) {
                for field in [&segment.label, &segment.detail, &segment.key] {
                    let id = field.as_ptr() as usize;
                    self.search_text.entry(id).or_insert_with(|| {
                        previous
                            .remove(&id)
                            .unwrap_or_else(|| (field.clone(), field.to_lowercase()))
                    });
                }
            }
        }
        self.filtered = self
            .all
            .iter()
            .map(|track| {
                (0..track.segments.len())
                    .filter(|&index| {
                        if search.trim().is_empty() {
                            return true;
                        }
                        let segment = &track.segments[index];
                        let fields = [&segment.label, &segment.detail, &segment.key]
                            .map(|field| self.search_text[&(field.as_ptr() as usize)].1.as_str());
                        if search.contains(' ') {
                            fields.join(" ").contains(&search)
                        } else {
                            fields.iter().any(|field| field.contains(&search))
                        }
                    })
                    .collect()
            })
            .collect();
        let old_visible = self.visible.clone();
        self.visible = self
            .order
            .iter()
            .copied()
            .filter(|index| self.shown[*index])
            .collect();
        if self.visible != old_visible {
            self.grid_cache.clear();
        }
        self.summary();
        self.hover = None;
        self.cache.clear();
    }
    fn summary(&mut self) {
        let track = [0, 2, 3][self.tab];
        self.rows = model::summarize_iter(
            self.filtered[track]
                .iter()
                .map(|i| &self.all[track].segments[*i]),
            self.selection.unwrap_or(self.day),
        );
        self.documents = if self.tab == 1 {
            self.highlight
                .as_deref()
                .map(|key| {
                    model::application_documents_iter(
                        self.filtered[3].iter().map(|i| &self.all[3].segments[*i]),
                        key,
                        self.selection.unwrap_or(self.day),
                    )
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    }
    fn change_date(&mut self, date: NaiveDate) {
        self.report_ui.highlight = None;
        self.date = date;
        self.date_text = date.to_string();
        self.day = model::day_bounds(date);
        self.view = self.day;
        self.grid_cache.clear();
        self.selection = None;
        self.highlight = None;
        self.limit = 50;
        if self.fixture {
            self.search_text.clear();
            self.all = model::fixture_with_apps(self.day, &self.preview_apps);
        } else {
            self.all = model::empty_tracks();
            self.invalidate_load();
        }
        self.rebuild();
    }
    fn tick_demand(&self) -> ticking::TickDemand {
        let now = draugr_time_daemon::now_ms();
        ticking::TickDemand {
            always: self.lifecycle || self.benchmark || cfg!(target_os = "macos"),
            window_open: self.window.is_some(),
            window_diagnostics: self.smoke || self.runtime_test,
            can_load: !self.fixture && self.runtime.is_some(),
            day_loaded: self.loaded_day == Some(self.day),
            live_day: self.day.start <= now && now < self.day.end,
            on_day_screen: self.screen == "Day",
        }
    }
    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            Subscription::run(tray::events).map(Message::Tray),
            window::close_requests().map(Message::Close),
            window::close_events().map(|_| Message::Closed),
            iced::event::listen_with(|event, status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                    if status == iced::event::Status::Ignored
                        || (modifiers.command()
                            && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("f"))) =>
                {
                    Some(Message::Key(key, modifiers))
                }
                _ => None,
            }),
        ];
        if self.tick_demand().needed() {
            subscriptions.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick));
        }
        // The portal reports theme changes itself; listen only while there is a window to theme.
        #[cfg(target_os = "linux")]
        if self.theme_preference == appearance::Preference::System && self.window.is_some() {
            subscriptions.push(
                Subscription::run(appearance::portal_theme_changes)
                    .map(|mode| Message::PortalThemeChanged(Some(mode))),
            );
        }
        #[cfg(not(target_os = "linux"))]
        subscriptions.push(iced::system::theme_changes().map(Message::SystemThemeChanged));
        Subscription::batch(subscriptions)
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Report(event) => return self.edit_report(event),
            Message::Edit(event) => return self.edit(event),
            Message::Rule(event) => {
                self.edit_rule(event);
            }
            Message::Language(choice) => {
                match localization::path().and_then(|p| localization::save(&p, choice)) {
                    Ok(()) => {
                        self.language_choice = choice;
                        localization::set(localization::resolve(
                            choice,
                            &localization::SystemLocale.locale(),
                        ));
                        self.cache.clear();
                        self.grid_cache.clear();
                        if let Some(tray) = &self.tray {
                            tray.update(&self.status, self.metrics[0], self.fixture);
                        }
                    }
                    Err(error) => self.error = error.to_string(),
                }
            }
            Message::Tray(tray::Event::Menu(event)) => {
                let message = match event.as_str() {
                    "open" => return self.open(true),
                    "settings" => {
                        self.set_screen("Settings");
                        return self.open(true);
                    }
                    "start" => Message::Start,
                    "pause" => Message::Pause(None),
                    "pause15" => Message::Pause(Some(900_000)),
                    "resume" => Message::Resume,
                    "quick5" => Message::Edit(tagging::Edit::Quick(5)),
                    "quick15" => Message::Edit(tagging::Edit::Quick(15)),
                    "quick60" => Message::Edit(tagging::Edit::Quick(60)),
                    "quit" => Message::Quit,
                    _ => return Task::none(),
                };
                return self.update(message);
            }
            Message::Tray(tray::Event::Status(status)) => {
                let returned = self.status.computer.activity != "active"
                    && status.computer.activity == "active";
                self.status = (*status).clone();
                if returned && self.observe_away() {
                    return self.open(true);
                }
                if let Some(tray) = &self.tray {
                    tray.update(&self.status, self.metrics[0], self.fixture);
                }
            }
            Message::Settings(edit) => return self.edit_settings(edit),
            Message::Tick => {
                if self.onboarding_test
                    && !self.settings_ui.preferences.onboarded
                    && self.clock.elapsed()
                        > Duration::from_secs((self.settings_ui.step + 1) as u64)
                {
                    return self.edit_settings(settings::Edit::Next);
                }

                if self.last_sample.elapsed() >= Duration::from_secs(1) {
                    let was_active = self.status.computer.activity == "active";
                    self.last_sample = Instant::now();
                    if let Some(runtime) = self.runtime.as_mut() {
                        #[cfg(target_os = "macos")]
                        runtime.pulse();
                        if let Some(status) = runtime.status() {
                            self.status = status;
                        }
                    }
                    if !was_active
                        && self.status.computer.activity == "active"
                        && self.observe_away()
                    {
                        return self.open(true);
                    }
                    if let Some(tray) = self.tray.as_ref() {
                        tray.update(&self.status, self.metrics[0], self.fixture);
                    }
                }
                let painted = self.stats.frames.load(Ordering::Relaxed) > 0;
                if self.lifecycle && painted {
                    let recorded = self.all[2]
                        .segments
                        .iter()
                        .map(|s| s.range.duration())
                        .sum::<i64>();
                    if self.cycle_stage == 0
                        && self.clock.elapsed() > Duration::from_secs(3)
                        && recorded > 0
                    {
                        self.cycle_duration = recorded;
                        self.cycle_stage = 1;
                        if let Some(id) = self.window {
                            return self.update(Message::Close(id));
                        }
                    }
                    if self.cycle_stage == 1 && self.clock.elapsed() > Duration::from_secs(6) {
                        self.cycle_stage = 2;
                        self.cycle_frames = self.stats.frames.load(Ordering::Relaxed);
                        return self.open(true);
                    }
                    if self.cycle_stage == 2
                        && self.stats.frames.load(Ordering::Relaxed) > self.cycle_frames
                        && self.clock.elapsed() > Duration::from_secs(8)
                        && recorded >= self.cycle_duration + 2_000
                    {
                        TEST_PASSED.store(true, Ordering::Relaxed);
                        eprintln!("Iced lifecycle passed: tray created, window closed, recording continued, window reopened");
                        return iced::exit();
                    }
                }
                if self.smoke
                    && !TEST_PASSED.load(Ordering::Relaxed)
                    && painted
                    && self.clock.elapsed() > Duration::from_secs(2)
                {
                    TEST_PASSED.store(true, Ordering::Relaxed);
                    eprintln!(
                        "Iced smoke passed: 5000 fixture events, canvas build max {:.2} ms",
                        self.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                    );
                    return iced::exit();
                }
                if self.runtime_test
                    && !self.lifecycle
                    && painted
                    && self.all[2].segments.iter().any(|s| s.range.duration() > 0)
                {
                    if self.onboarding_test {
                        let persisted = draugr_time_daemon::paths::data_dir()
                            .ok()
                            .and_then(|p| {
                                draugr_time_daemon::preferences::Preferences::load(
                                    &p.join("settings.json"),
                                )
                                .ok()
                            })
                            .is_some_and(|p| p.onboarded);
                        assert!(persisted, "onboarding must persist before showing Day");
                        eprintln!("Iced onboarding passed: two native screens, saved completion, working Day view");
                    }
                    TEST_PASSED.store(true, Ordering::Relaxed);
                    eprintln!("Iced runtime passed: native window and embedded SQLite recording");
                    return iced::exit();
                }
                if (self.smoke || self.runtime_test)
                    && self.clock.elapsed() > Duration::from_secs(20)
                {
                    eprintln!("Iced smoke/runtime check timed out");
                    std::process::exit(1);
                }
                if self.benchmark
                    && painted
                    && !self.snapshot_done
                    && self.clock.elapsed() > Duration::from_secs(3)
                {
                    self.snapshot_done = true;
                    if let Some(id) = self.window {
                        return window::screenshot(id).map(Message::Screenshot);
                    }
                }
                if self.benchmark
                    && !self.closed_for_benchmark
                    && self.clock.elapsed() > Duration::from_secs(7)
                {
                    self.closed_for_benchmark = true;
                    if let Some(id) = self.window.take() {
                        self.release_day();
                        return window::close(id);
                    }
                }
                if self.benchmark && self.clock.elapsed() > Duration::from_secs(12) {
                    eprintln!(
                        "Iced benchmark: canvas build max {:.2} ms",
                        self.stats.max_build_us.load(Ordering::Relaxed) as f64 / 1000.0
                    );
                    return iced::exit();
                }
                let now = draugr_time_daemon::now_ms();
                let today = self.day.start <= now && now < self.day.end;
                // Only today's history still changes; a past day is loaded once.
                let stale = self.loaded_day != Some(self.day)
                    || (today && self.last_load.elapsed() > Duration::from_secs(2));
                if !self.fixture && !self.loading && stale && self.window.is_some() {
                    if let Some(runtime) = self.runtime.as_ref() {
                        let day = self.day;
                        let cursor = self.load_cursor.clone();
                        let generation = self.load_generation;
                        self.loading = true;
                        self.last_load = Instant::now();
                        return Task::perform(runtime.loader.load(day, cursor), move |result| {
                            Message::Loaded(generation, result)
                        });
                    }
                }
            }
            Message::Loaded(generation, result) => {
                if generation != self.load_generation {
                    // A closed window must not retain the result or the allocator's freed pages.
                    drop(result);
                    if self.window.is_none() {
                        memory::SystemReclaimer.reclaim();
                    }
                    return Task::none();
                }
                self.loading = false;
                match result {
                    Ok(snapshot) => {
                        if self.away_range.is_none() {
                            self.away_range = self.away_prompt.observe(snapshot.completed_afk);
                        }
                        if let Some(metadata) = snapshot.tag_metadata {
                            self.tags = metadata.tags;
                            self.tag_ranges = metadata.ranges;
                            self.recent_tags = metadata.recent;
                        }
                        self.loaded_day = Some(self.day);
                        let new_icons: HashMap<_, _> = snapshot
                            .icons
                            .into_iter()
                            .filter(|(key, _)| !self.icons.contains_key(key))
                            .collect();
                        self.icons.extend(decode_icons(new_icons));
                        if self.editor.is_none() {
                            self.error.clear();
                        }
                        self.load_cursor = *snapshot.cursor;
                        let mut changed = false;
                        for (index, (track, incoming)) in
                            self.all.iter_mut().zip(snapshot.tracks).enumerate()
                        {
                            if index == 0 {
                                if snapshot.tags_changed {
                                    track.segments.retain(|s| s.bucket == "derived");
                                    track.segments.extend(incoming.segments);
                                    self.search_text.clear();
                                    changed = true;
                                }
                                continue;
                            }
                            let split = track
                                .segments
                                .iter()
                                .position(|s| {
                                    snapshot
                                        .replace_from
                                        .get(s.bucket)
                                        .is_some_and(|from| s.range.start >= *from)
                                })
                                .unwrap_or(track.segments.len());
                            let mut tail: Vec<_> = track.segments[split..]
                                .iter()
                                .filter(|s| {
                                    s.bucket != "derived"
                                        && snapshot
                                            .replace_from
                                            .get(s.bucket)
                                            .is_none_or(|from| s.range.start < *from)
                                })
                                .cloned()
                                .collect();
                            tail.extend(incoming.segments);
                            tail.sort_by_key(|s| (s.range.start, s.range.end));
                            // Compare only the fetched tail; history is never cloned or compared.
                            let old: Vec<_> = track.segments[split..]
                                .iter()
                                .filter(|s| s.bucket != "derived")
                                .collect();
                            if !old.iter().copied().eq(tail.iter()) {
                                track.segments.truncate(split);
                                track.segments.extend(tail);
                                changed = true;
                            }
                        }
                        if changed {
                            model::derive_tracks(&mut self.all, self.day);
                            self.rebuild();
                        }
                    }
                    Err(error) => self.error = error,
                }
            }
            Message::Opened(id) => self.window = Some(id),
            Message::Close(id) => {
                if self.tray.is_some() {
                    self.window = None;
                    self.release_day();
                    return window::close(id);
                }
                return iced::exit();
            }
            Message::Closed => memory::SystemReclaimer.reclaim(),
            Message::ResizeDay(event) => {
                if event.ratio.is_finite() {
                    self.day_panes
                        .resize(event.split, event.ratio.clamp(0.0, 1.0));
                }
            }
            Message::Screen(screen) => {
                self.set_screen(screen);
                if matches!(screen, "Overview" | "Week" | "Reports") {
                    return self.edit_report(reports::Edit::Refresh);
                }
            }
            Message::Date(input) => {
                self.date_text = input.clone();
                if let Ok(date) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
                    self.change_date(date);
                    if self.screen == "Week" {
                        return self.edit_report(reports::Edit::Refresh);
                    }
                }
            }
            Message::Shift(delta) => {
                let date = if delta < 0 {
                    self.date
                        .checked_sub_days(Days::new(if self.screen == "Week" { 7 } else { 1 }))
                } else {
                    self.date
                        .checked_add_days(Days::new(if self.screen == "Week" { 7 } else { 1 }))
                };
                if let Some(date) = date {
                    self.change_date(date);
                    if self.screen == "Week" {
                        return self.edit_report(reports::Edit::Refresh);
                    }
                }
            }
            Message::Today => {
                self.change_date(Local::now().date_naive());
                if self.screen == "Week" {
                    return self.edit_report(reports::Edit::Refresh);
                }
            }
            Message::Search(search) => {
                self.search = search;
                self.limit = 50;
                self.rebuild();
            }
            Message::Tab(tab) => {
                self.tab = tab;
                self.highlight = None;
                self.limit = 50;
                self.cache.clear();
                self.grid_cache.clear();
                self.summary();
            }
            Message::Select(selection) => {
                self.selection = selection;
                self.highlight = None;
                self.limit = 50;
                self.summary();
            }
            Message::Hover(hover) => self.hover = hover,
            Message::Highlight(key) => {
                self.highlight = if self.highlight.as_ref() == Some(&key) {
                    None
                } else {
                    Some(key)
                };
                self.summary();
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Zoom(factor, anchor) => {
                let anchor = anchor.or_else(|| self.selection.map(|s| s.start + s.duration() / 2));
                self.view = model::zoom(self.day, self.view, factor, anchor);
                self.hover = None;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Fit => {
                self.view = self.day;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::Pan(value) => {
                let span = self.view.duration();
                let start = self.day.start
                    + (value.clamp(0.0, 1.0) * (self.day.duration() - span) as f64).round() as i64;
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.grid_cache.clear();
                self.hover = None;
            }
            Message::PanBy(value) => {
                let span = self.view.duration();
                let start = (self.view.start + (value * span as f64) as i64)
                    .clamp(self.day.start, self.day.end - span);
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.grid_cache.clear();
                self.hover = None;
            }
            Message::Tracks => self.tracks_menu = !self.tracks_menu,
            Message::Visible(index, visible) => {
                self.shown[index] = visible;
                self.rebuild();
            }
            Message::Move(index, delta) => {
                let next = (index as i32 + delta) as usize;
                if next < self.order.len() {
                    self.order.swap(index, next);
                    self.rebuild();
                }
            }
            Message::More => self.limit += 50,
            Message::OpenUrl(url) => open_url(url),
            Message::Theme => {
                self.theme_preference = self.theme_preference.next();
                self.apply_theme();
            }
            #[cfg(not(target_os = "linux"))]
            Message::SystemThemeChanged(mode) => {
                let changed = self.system_theme != mode;
                self.system_theme = mode;
                if changed && self.theme_preference == appearance::Preference::System {
                    self.apply_theme();
                }
            }
            #[cfg(target_os = "linux")]
            Message::PortalThemeChanged(Some(mode)) => {
                let changed = self.system_theme != mode;
                self.system_theme = mode;
                if changed && self.theme_preference == appearance::Preference::System {
                    self.apply_theme();
                }
            }
            #[cfg(target_os = "linux")]
            Message::PortalThemeChanged(None) => {}
            Message::Start => self.start(),
            Message::AutoStart(enabled) => match startup::SystemStartup::new()
                .and_then(|mut provider| startup::set_enabled(&mut provider, enabled))
            {
                Ok(enabled) => {
                    self.autostart_enabled = enabled;
                    self.autostart_error.clear();
                }
                Err(error) => self.autostart_error = error,
            },
            Message::Pause(duration) => {
                if let Some(runtime) = self.runtime.as_ref() {
                    if let Err(error) = runtime.pause(duration) {
                        self.error = error;
                    }
                    if let Some(status) = runtime.status() {
                        self.status = status;
                    }
                }
            }
            Message::Resume => {
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.resume();
                    if let Some(status) = runtime.status() {
                        self.status = status;
                    }
                }
            }
            Message::Quit => return iced::exit(),
            Message::Folder => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_owned())
                    },
                    Message::FolderChosen,
                )
            }
            Message::FolderChosen(folder) => {
                if let Some(folder) = folder {
                    let result = draugr_time_daemon::paths::default_dir().and_then(|dir| {
                        draugr_time_daemon::paths::save_choice(
                            &dir.join("preferences.json"),
                            &folder,
                        )
                    });
                    match result {
                        Ok(()) => self.pending_dir = Some(folder.to_string_lossy().into_owned()),
                        Err(error) => self.error = error.to_string(),
                    }
                }
            }
            Message::Key(key, modifiers) => {
                if self.screen == "Overview" && self.editor.is_none() {
                    use keyboard::key::Named;
                    match &key {
                        keyboard::Key::Named(Named::ArrowDown | Named::ArrowUp) => {
                            let slices =
                                reports::slices(&self.report_ui.day, self.report_ui.expanded);
                            let mut values = Vec::new();
                            for slice in slices {
                                let project = (slice.tag_id, None);
                                if !values.contains(&project) {
                                    values.push(project);
                                }
                                values.push((slice.tag_id, Some(slice.application)));
                            }
                            if !values.is_empty() {
                                let previous = values.iter().position(|value| {
                                    self.report_ui.highlight.as_ref() == Some(value)
                                });
                                let index = if key == keyboard::Key::Named(Named::ArrowUp) {
                                    previous
                                        .map(|i| (i + values.len() - 1) % values.len())
                                        .unwrap_or(values.len() - 1)
                                } else {
                                    previous.map(|i| (i + 1) % values.len()).unwrap_or(0)
                                };
                                self.report_ui.highlight = Some(values[index].clone());
                            }
                            return Task::none();
                        }
                        keyboard::Key::Named(Named::Enter) => return Task::none(),
                        keyboard::Key::Named(Named::Escape) => {
                            self.report_ui.highlight = None;
                            return Task::none();
                        }
                        keyboard::Key::Character(c) if c.eq_ignore_ascii_case("e") => {
                            return self.edit_report(reports::Edit::Expand)
                        }
                        _ => {}
                    }
                }
                if self.screen == "Day" && self.editor.is_none() && modifiers.alt() {
                    let delta = match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => -0.05,
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => 0.05,
                        _ => 0.0,
                    };
                    if delta != 0.0 {
                        if let pane_grid::Node::Split { id, ratio, .. } = self.day_panes.layout() {
                            return self.update(Message::ResizeDay(pane_grid::ResizeEvent {
                                split: *id,
                                ratio: ratio + delta,
                            }));
                        }
                    }
                }
                if modifiers.command()
                    && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("z"))
                {
                    return self.edit(tagging::Edit::Undo);
                }
                if self.editor.is_some() {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            self.edit(tagging::Edit::Cancel)
                        }
                        keyboard::Key::Named(keyboard::key::Named::Enter)
                            if self.editor.as_ref().is_some_and(|e| !e.overlap) =>
                        {
                            self.edit(
                                if self.editor.as_ref().is_some_and(|e| e.tag_id.is_some()) {
                                    tagging::Edit::SaveTag
                                } else {
                                    tagging::Edit::Save(false)
                                },
                            )
                        }
                        _ => Task::none(),
                    };
                }
                if key == keyboard::Key::Named(keyboard::key::Named::Enter) {
                    return self.edit(tagging::Edit::Open);
                }
                if modifiers.command()
                    && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("f"))
                {
                    return widget::operation::focus("activity-search");
                }
                let message = match key {
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        Some(Message::Shift(-1))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        Some(Message::Shift(1))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(Message::Select(None))
                    }
                    keyboard::Key::Character(c) => match c.as_str() {
                        "t" | "T" => Some(Message::Today),
                        "+" | "=" => Some(Message::Zoom(0.9, None)),
                        "-" => Some(Message::Zoom(1.0 / 0.9, None)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(message) = message {
                    return self.update(message);
                }
            }
            Message::Screenshot(screenshot) => {
                if let Some(path) = std::env::var_os("DRAUGR_TIME_SCREENSHOT") {
                    let result = (|| -> Result<(), String> {
                        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
                        let mut encoder =
                            png::Encoder::new(file, screenshot.size.width, screenshot.size.height);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder
                            .write_header()
                            .map_err(|e| e.to_string())?
                            .write_image_data(&screenshot.rgba)
                            .map_err(|e| e.to_string())?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        eprintln!("Screenshot failed: {error}");
                    }
                }
            }
        }
        Task::none()
    }
    fn status_label(&self) -> String {
        tr(match self.status.state.as_str() {
            "tracking" => "● Tracking",
            "paused" => "● Paused",
            "away" => "● AFK",
            "asleep" => "● Sleep",
            "locked" => "● Locked",
            "error" => "● Needs attention",
            "starting" => "● Starting",
            _ => "○ Not recording",
        })
    }
    fn app_icon<'a>(&'a self, app: &str, app_name: &str) -> Element<'a, Message> {
        match cached_app_icon(&self.icons, app, app_name) {
            Some(AppIcon::Png(handle)) => image(handle.clone()).width(20).height(20).into(),
            Some(AppIcon::Svg(handle)) => svg(handle.clone()).width(20).height(20).into(),
            None => {
                let rgb = model::color(app_name);
                container(
                    text(
                        app_name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string(),
                    )
                    .size(11),
                )
                .width(20)
                .height(20)
                .center(20)
                .style(move |_: &Theme| container::Style {
                    background: Some(Color::from_rgb(rgb[0], rgb[1], rgb[2]).into()),
                    text_color: Some(Color::WHITE),
                    border: iced::Border::default().rounded(4),
                    ..container::Style::default()
                })
                .into()
            }
        }
    }
    fn day_view(&self) -> Element<'_, Message> {
        let mut metrics = row![].spacing(10);
        for (label, value) in [
            "ACTIVE TIME",
            "AWAY TIME",
            "TAGGED TIME",
            "UNTAGGED ACTIVITY",
        ]
        .into_iter()
        .zip(self.metrics)
        {
            metrics = metrics.push(
                container(
                    column![
                        text(tr(label)).size(10),
                        text(model::duration(value)).size(24)
                    ]
                    .spacing(2),
                )
                .width(Length::Fill)
                .padding(10)
                .style(panel),
            );
        }
        let mode = if self.fixture {
            "Preview · 5,000 generated events · sample data is never saved"
        } else {
            "Recorded activity"
        };
        let panes = pane_grid::PaneGrid::new(&self.day_panes, |_, pane, _| {
            pane_grid::Content::new(match pane {
                DayPane::Timeline => self.day_timeline(),
                DayPane::Summary => self.day_summary(),
            })
        })
        .spacing(10)
        .min_size(160)
        .on_resize(6, Message::ResizeDay);
        column![
            metrics,
            row![
                text(tr(mode)).size(12),
                space::horizontal(),
                text(tr("Drag divider to resize · Alt ↑/↓")).size(10)
            ],
            panes
        ]
        .spacing(10)
        .height(Length::Fill)
        .into()
    }
    fn day_timeline(&self) -> Element<'_, Message> {
        let heading = row![
            text(tr("Activity timeline")).size(15),
            space::horizontal(),
            button(text(tr("Tracks"))).on_press(Message::Tracks),
            button(text(tr("−"))).on_press(Message::Zoom(1.0 / 0.9, None)),
            text(if self.view == self.day {
                tr("Full day")
            } else {
                model::duration(self.view.duration())
            })
            .size(12),
            button(text(tr("+"))).on_press(Message::Zoom(0.9, None)),
            button(text(tr("Fit day"))).on_press(Message::Fit)
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut timeline_body = column![heading].spacing(8);
        if self.tracks_menu {
            let mut menu = column![].spacing(4);
            for (index, track) in self.order.iter().enumerate() {
                let up = (index > 0).then_some(Message::Move(index, -1));
                let down = (index + 1 < self.order.len()).then_some(Message::Move(index, 1));
                menu = menu.push(
                    row![
                        checkbox(self.shown[*track])
                            .label(tr(self.all[*track].name))
                            .on_toggle(move |value| Message::Visible(*track, value)),
                        space::horizontal(),
                        button(text(tr("↑"))).on_press_maybe(up),
                        button(text(tr("↓"))).on_press_maybe(down)
                    ]
                    .spacing(8),
                );
            }
            timeline_body = timeline_body.push(menu);
        }
        let labels = self.visible.iter().fold(
            column![container(text(tr("LOCAL TIME")).size(10))
                .height(timeline::RULER)
                .center_y(timeline::RULER)],
            |column, index| {
                column.push(
                    container(text(tr(self.all[*index].name)).size(12))
                        .height(timeline::ROW)
                        .center_y(timeline::ROW),
                )
            },
        );
        let plot = timeline::canvas(timeline::Timeline {
            tracks: &self.all,
            filtered: &self.filtered,
            visible: &self.visible,
            view: self.view,
            selection: self.selection,
            highlight: self.highlight.as_deref(),
            grid_cache: &self.grid_cache,
            cache: &self.cache,
            stats: &self.stats,
            icons: &self.icons,
        });
        let chart: Element<'_, Message> = if let Some(segment) = self
            .hover
            .and_then(|(track, index)| self.all.get(track)?.segments.get(index))
        {
            let details = column![
                text(localization::segment_label(segment)).size(14),
                text(
                    if matches!(segment.bucket, "computer" | "afk" | "derived") {
                        localization::error(&segment.detail)
                    } else {
                        segment.detail.to_string()
                    }
                )
                .size(12),
                text(format!(
                    "{}–{} · {}",
                    model::time_label(segment.range.start),
                    model::time_label(segment.range.end),
                    model::duration(segment.range.duration())
                ))
                .size(12)
            ]
            .spacing(5);
            let details: Element<'_, Message> = if let (Some(app), Some(app_name)) =
                (segment.app.as_deref(), segment.app_name.as_deref())
            {
                row![self.app_icon(app, app_name), details]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into()
            } else {
                details.into()
            };
            tooltip(
                plot,
                container(details).padding(10),
                tooltip::Position::FollowCursor,
            )
            .style(panel)
            .into()
        } else {
            plot.into()
        };
        let pan = (self.view.start - self.day.start) as f64
            / (self.day.duration() - self.view.duration()).max(1) as f64;
        timeline_body=timeline_body.push(row![container(labels).width(124),chart].spacing(8)).push(row![text(tr("Pan across day")).size(10).width(124),slider(0.0..=1.0,pan,Message::Pan).step(0.001)].spacing(8)).push(text(tr("Active · AFK · Sleep · Locked · Paused · No recording · Unobserved (diagnostic)   |   Drag to select · Shift-click to extend · Double-click a block · Wheel to zoom")).size(10));
        container(
            scrollable(timeline_body.padding(iced::Padding {
                right: 12.0,
                ..iced::Padding::ZERO
            }))
            .height(Length::Fill),
        )
        .padding(12)
        .height(Length::Fill)
        .style(panel)
        .into()
    }
    fn day_summary(&self) -> Element<'_, Message> {
        let selection = self
            .selection
            .map(|s| {
                localized(
                    "Selection {start}–{end} · {duration}",
                    &[
                        ("start", model::time_label(s.start)),
                        ("end", model::time_label(s.end)),
                        ("duration", model::duration(s.duration())),
                    ],
                )
            })
            .unwrap_or_else(|| tr("Whole day"));
        let selection_bar = row![
            text(selection).size(14),
            space::horizontal(),
            button(text(tr("Clear"))).on_press(Message::Select(None)),
            button(text(tr("Tag selection ⏎")))
                .on_press_maybe(self.selection.map(|_| Message::Edit(tagging::Edit::Open)))
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let tabs = ["Tags", "Applications", "Documents"]
            .into_iter()
            .enumerate()
            .fold(row![].spacing(6), |row, (index, label)| {
                row.push(button(text(tr(label))).on_press(Message::Tab(index)).style(
                    if index == self.tab {
                        button::primary
                    } else {
                        button::secondary
                    },
                ))
            });
        let total = self.rows.iter().map(|row| row.duration).sum();
        let tabs = row![
            tabs,
            space::horizontal(),
            text(localized(
                "{duration} observed",
                &[("duration", model::duration(total))]
            ))
            .size(12)
        ]
        .align_y(Alignment::Center);
        let mut body = column![].spacing(3);
        for entry in self.rows.iter().take(self.limit) {
            let rgb = entry.color;
            let icon: Element<'_, Message> = if let (Some(app), Some(app_name)) =
                (entry.app.as_deref(), entry.app_name.as_deref())
            {
                self.app_icon(app, app_name)
            } else {
                container(space::horizontal())
                    .width(10)
                    .height(10)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Color::from_rgb(rgb[0], rgb[1], rgb[2]).into()),
                        ..container::Style::default()
                    })
                    .into()
            };
            let mut label = row![].spacing(8);
            if self.tab == 1 {
                label = label.push(
                    text(if self.highlight.as_deref() == Some(entry.key.as_ref()) {
                        "▾"
                    } else {
                        "▸"
                    })
                    .size(13),
                );
            }
            let label = label
                .push(icon)
                .push(
                    text(if self.tab == 0 && entry.key.as_ref() == "untagged" {
                        tr("Untagged")
                    } else {
                        entry.label.to_string()
                    })
                    .size(13),
                )
                .spacing(8)
                .align_y(Alignment::Center);
            let style = if self.highlight.as_deref() == Some(entry.key.as_ref()) {
                button::primary
            } else {
                button::text
            };
            body = body.push(
                row![
                    button(label)
                        .on_press(Message::Highlight(entry.key.to_string()))
                        .style(style)
                        .width(Length::Fill),
                    text(model::duration(entry.duration)).size(13).width(90),
                    text(format!("{:.1}%", entry.share)).size(13).width(78)
                ]
                .align_y(Alignment::Center),
            );
            if self.tab == 1 && self.highlight.as_deref() == Some(entry.key.as_ref()) {
                let mut documents =
                    column![text(tr("Documents / pages · most time first")).size(11)].spacing(6);
                for document in &self.documents {
                    documents = documents.push(
                        row![
                            text(document.label.as_ref()).size(12).width(Length::Fill),
                            text(model::duration(document.duration)).size(12).width(90),
                            text(format!("{:.1}%", document.share)).size(12).width(78)
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
                }
                if self.documents.is_empty() {
                    documents = documents.push(
                        text(tr(
                            "No recorded document titles for this application in this range.",
                        ))
                        .size(12),
                    );
                }
                body = body.push(
                    container(documents)
                        .padding(iced::Padding {
                            top: 4.0,
                            right: 0.0,
                            bottom: 8.0,
                            left: 36.0,
                        })
                        .width(Length::Fill),
                );
            }
        }
        if self.rows.is_empty() {
            body = body.push(
                text(tr(if self.search.is_empty() {
                    "No activity in this range yet."
                } else {
                    "No matching activity."
                }))
                .size(13),
            );
        }
        if self.rows.len() > self.limit {
            body = body.push(button(text(tr("Show 50 more"))).on_press(Message::More));
        }
        container(column![selection_bar,tabs,row![text(tr("Project / application / document")).size(10).width(Length::Fill),text(tr("Duration")).size(10).width(90),text(tr("Range share")).size(10).width(78)],scrollable(body).height(Length::Fill),text(tr("Shares refer to the selected time range. Overlaps within one item are counted once.")).size(10)].spacing(7)).padding(12).height(Length::Fill).style(panel).into()
    }
    fn settings(&self) -> Element<'_, Message> {
        let mut backend=column![text(tr("Status & permissions")).size(22),text(tr("Live collector status. macOS and Wayland still require OS permissions/integration.")).size(13)].spacing(12);
        if self.runtime.is_some() {
            let computer = &self.status.computer;
            backend = backend.push(
                text(localized(
                    "Computer: {machine} · Activity: {activity} · Recording: {recording}",
                    &[
                        ("machine", localization::state(&computer.machine)),
                        ("activity", localization::state(&computer.activity)),
                        ("recording", localization::state(&computer.recording)),
                    ],
                ))
                .size(13),
            );
            if let Some(reason) = &computer.reason {
                backend = backend.push(text(localization::error(reason)).size(13));
            }
            backend = backend
                .push(text(localization::error(&self.status.computer_capabilities)).size(12));
            for value in &self.status.backends {
                backend = backend.push(
                    text(format!(
                        "{} · {}",
                        tr(&value.name),
                        localization::state(&value.state)
                    ))
                    .size(15),
                );
                if let Some(error) = &value.error {
                    backend = backend.push(text(localization::error(error)).size(13));
                }
            }
        }
        if self.runtime.is_none() {
            backend = backend.push(text(tr(
                "Tracking is stopped. Press Start recording to collect activity.",
            )));
        }
        let directory = draugr_time_daemon::paths::data_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|e| e.to_string());
        let folder = column![
            text(tr("Data folder")).size(22),
            text(directory).size(13),
            button(text(tr("Choose folder"))).on_press(Message::Folder),
            button(text(tr("Import ActivityWatch / Draugr JSON"))).on_press(Message::Report(reports::Edit::Import)),
            text(tr("Import adds history atomically; overlapping event buckets are rejected. JSON export is available in Reports.")),
            text(
                self.pending_dir
                    .as_ref()
                    .map(|path| localized(
                        "Next launch: {path}. Existing history stays in the old folder.",
                        &[("path", path.clone())]
                    ))
                    .unwrap_or_else(|| {
                        tr("The folder change takes effect after restarting Draugr Time.")
                    })
            )
            .size(12)
        ]
        .spacing(12);
        let autostart = column![
            checkbox(self.autostart_enabled)
                .label(tr("Start Draugr Time when I log in"))
                .on_toggle(Message::AutoStart),
            text(tr("Starts activity tracking in the tray. Turn this off here to remove it from system startup."))
                .size(12),
            text(localization::error(&self.autostart_error))
                .size(12)
                .color(Color::from_rgb8(200, 75, 75))
        ]
        .spacing(8);
        let resource=column![image(image::Handle::from_bytes(crate::branding::logo(self.theme.palette().background.r < 0.5))).width(64).height(64).filter_method(image::FilterMethod::Nearest),text(tr("Draugr Time")).size(22),text(concat!("Version ", env!("CARGO_PKG_VERSION"), " · GPL-3.0-only")),button(text("https://github.com/hlophlopgaming/DraugrTime")).on_press(Message::OpenUrl("https://github.com/hlophlopgaming/DraugrTime")),row![text(tr("Created by")),button(text("hlophlopgaming")).on_press(Message::OpenUrl("https://github.com/hlophlopgaming"))].spacing(6).align_y(Alignment::Center),text(tr("Native Rust UI · tiny-skia software renderer")).size(14),text(localized("Canvas draws: {count} · max geometry build: {duration} ms",&[("count",self.stats.frames.load(Ordering::Relaxed).to_string()),("duration",format!("{:.2}",self.stats.max_build_us.load(Ordering::Relaxed) as f64/1000.0))])).size(13),text(tr("Closing the window releases its renderer and keeps recording. Open it again from the tray. Quit stops the process. Themes, tray behavior and OS permissions need verification on each platform.")).size(13)].spacing(12);
        scrollable(
            column![
                container({
                    let mut languages = column![
                        button(text(tr("System default"))).on_press(Message::Language(None)),
                        text(localized(
                            "Language: {language}",
                            &[("language", localization::current().name().into())]
                        ))
                    ]
                    .spacing(6);
                    for language in localization::Language::ALL {
                        languages = languages.push(
                            button(language.name()).on_press(Message::Language(Some(language))),
                        );
                    }
                    languages
                })
                .padding(16)
                .style(panel),
                container(self.privacy_settings()).padding(16).style(panel),
                container(backend).padding(16).style(panel),
                container(folder).padding(16).style(panel),
                container(autostart).padding(16).style(panel),
                container(
                    column![
                        text(tr("While you were away")).size(22),
                        text(localized(
                            "Prompt threshold: {duration}",
                            &[(
                                "duration",
                                if self.away_prompt.threshold > 86_400_000 {
                                    tr("disabled")
                                } else {
                                    model::duration(self.away_prompt.threshold)
                                }
                            )]
                        )),
                        row![
                            button(text(tr("15 min")))
                                .on_press(Message::Edit(tagging::Edit::Threshold(15))),
                            button(text(tr("30 min")))
                                .on_press(Message::Edit(tagging::Edit::Threshold(30))),
                            button(text(tr("60 min")))
                                .on_press(Message::Edit(tagging::Edit::Threshold(60))),
                            button(text(tr("Off"))).on_press(Message::Edit(
                                tagging::Edit::Threshold(i64::MAX / 60_000)
                            ))
                        ]
                        .spacing(8)
                    ]
                    .spacing(8)
                )
                .padding(16)
                .style(panel),
                container(resource).padding(16).style(panel)
            ]
            .spacing(14),
        )
        .into()
    }
    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        if !self.settings_ui.preferences.onboarded
            && !self.fixture
            && (!self.runtime_test || self.onboarding_test)
        {
            return self.onboarding();
        }
        let mut navigation = column![
            image(image::Handle::from_bytes(crate::branding::logo(
                self.theme.palette().background.r < 0.5
            )))
            .width(72)
            .height(72)
            .filter_method(image::FilterMethod::Nearest),
            text("Draugr Time").size(23),
            text(tr("LOCAL ACTIVITY TRACKER")).size(10),
            space::vertical().height(14)
        ]
        .spacing(8);
        for screen in [
            "Day", "Overview", "Week", "Reports", "Tags", "Rules", "Settings",
        ] {
            navigation = navigation.push(
                button(text(tr(screen)))
                    .on_press(Message::Screen(screen))
                    .width(Length::Fill)
                    .style(if screen == self.screen {
                        button::primary
                    } else {
                        button::text
                    }),
            );
        }
        navigation = navigation
            .push(space::vertical())
            .push(text(tr("Local & private")).size(13))
            .push(text(tr("Activity stays on this computer.")).size(11));
        let sidebar = container(navigation)
            .padding(18)
            .width(184)
            .height(Length::Fill)
            .style(panel);
        let date = row![
            button(text(tr("‹"))).on_press(Message::Shift(-1)),
            text_input(&tr("YYYY-MM-DD"), &self.date_text)
                .on_input(Message::Date)
                .width(130),
            button(text(tr("›"))).on_press(Message::Shift(1)),
            button(text(tr("Today"))).on_press(Message::Today)
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        let toolbar = row![
            date,
            space::horizontal(),
            text_input(&tr("Search activity · Ctrl/Cmd F"), &self.search)
                .on_input(Message::Search)
                .id("activity-search")
                .width(250)
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let control: Element<'_, Message> = if self.runtime.is_none() {
            button(text(tr("Start recording")))
                .on_press(Message::Start)
                .into()
        } else if self.status.state == "paused" {
            button(text(tr("Resume"))).on_press(Message::Resume).into()
        } else {
            row![
                button(text(tr("Pause"))).on_press(Message::Pause(None)),
                button(text(tr("15 min"))).on_press(Message::Pause(Some(900_000)))
            ]
            .spacing(6)
            .into()
        };
        let heading = row![
            column![
                text(tr(if self.screen == "Day" {
                    "Day overview"
                } else {
                    self.screen
                }))
                .size(25),
                text(localization::date_label(self.date)).size(12)
            ]
            .spacing(2),
            space::horizontal(),
            text(self.status_label()).size(13),
            control,
            button(text(localized(
                "Theme: {theme}",
                &[("theme", tr(self.theme_preference.label()))]
            )))
            .on_press(Message::Theme),
            button(text(tr("Quit"))).on_press(Message::Quit)
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        let content = if self.screen == "Day" {
            self.day_view()
        } else if matches!(self.screen, "Overview" | "Week" | "Reports") {
            self.reports_view()
        } else if self.screen == "Tags" {
            self.tags_view()
        } else if self.screen == "Rules" {
            self.rules_view()
        } else if self.screen == "Settings" {
            self.settings()
        } else {
            container(
                column![
                    text(localized(
                        "{screen} is coming in a later milestone",
                        &[("screen", tr(self.screen))]
                    ))
                    .size(22),
                    text(tr("This view will be available in a later milestone.")),
                    button(text(tr("Back to day"))).on_press(Message::Screen("Day"))
                ]
                .spacing(14),
            )
            .padding(24)
            .into()
        };
        let mut main = column![toolbar, heading].spacing(10);
        main = main.push(
            button(text(tr("Undo tag action · Ctrl/Cmd Z"))).on_press_maybe(
                (!self.tag_undo.is_empty()).then_some(Message::Edit(tagging::Edit::Undo)),
            ),
        );
        if let Some(range) = self.away_range {
            main = main.push(
                container(
                    column![
                        text(localized(
                            "You were away {start}–{end} ({duration}). What were you doing?",
                            &[
                                ("start", model::time_label(range.start)),
                                ("end", model::time_label(range.end)),
                                ("duration", model::duration(range.duration()))
                            ]
                        )),
                        row![
                            button(text(tr("Tag away time")))
                                .on_press(Message::Edit(tagging::Edit::AwayTag)),
                            button(text(tr("Ignore")))
                                .on_press(Message::Edit(tagging::Edit::IgnoreAway)),
                            button(text(tr("Only ask after 30 min")))
                                .on_press(Message::Edit(tagging::Edit::Threshold(30))),
                            button(text(tr("Disable prompts"))).on_press(Message::Edit(
                                tagging::Edit::Threshold(i64::MAX / 60_000)
                            ))
                        ]
                        .spacing(8)
                    ]
                    .spacing(8),
                )
                .padding(10)
                .style(panel),
            );
        }
        if self.away_range.is_some() {
            let mut recent = row![].spacing(6);
            let mut ids = Vec::new();
            for &id in &self.recent_tags {
                if !ids.contains(&id) && self.tags.iter().any(|t| t.id == id && t.archived != 1) {
                    ids.push(id);
                    let name = model::tag_path(&self.tags, id);
                    recent = recent.push(
                        button(text(name.clone()))
                            .on_press(Message::Edit(tagging::Edit::AwayRecent(name))),
                    );
                    if ids.len() == 4 {
                        break;
                    }
                }
            }
            main = main.push(recent);
        }
        if !self.error.is_empty() {
            main = main.push(
                text(localization::error(&self.error))
                    .size(13)
                    .color(Color::from_rgb8(200, 75, 75)),
            );
        } else if self.status.state == "error" {
            let error = self
                .status
                .backends
                .iter()
                .find_map(|backend| backend.error.as_deref())
                .unwrap_or("Tracking needs attention; check Settings.");
            main = main.push(
                button(text(localization::error(error)).size(12))
                    .on_press(Message::Screen("Settings"))
                    .style(button::danger),
            );
        }
        main = main
            .push(content)
            .push(row![
                text(self.status_label()).size(11),
                space::horizontal(),
                text(tr("Draugr Time · Local & private")).size(11)
            ])
            .height(Length::Fill);
        // Track the background as a quad so tiny-skia repaints reused buffers after theme changes.
        let base: Element<'_, Message> = container(row![
            sidebar,
            container(main).padding(16).height(Length::Fill)
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.palette().background.into()),
            ..Default::default()
        })
        .into();
        if self.editor.is_some() {
            let dialog = container(scrollable(column![
                self.tag_editor(),
                text(localization::error(&self.error)).color(Color::from_rgb8(200, 75, 75))
            ]))
            .width(650)
            .max_height(520)
            .padding(10)
            .style(panel);
            widget::stack![
                base,
                widget::opaque(container(dialog).center(Length::Fill).style(|_| {
                    container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.35).into()),
                        ..Default::default()
                    }
                }))
            ]
            .into()
        } else {
            base
        }
    }
}

fn native_theme(dark: bool) -> Theme {
    Theme::custom(
        "Draugr Time",
        iced::theme::Palette {
            background: if dark {
                Color::from_rgb8(14, 25, 35)
            } else {
                Color::from_rgb8(244, 246, 249)
            },
            text: if dark {
                Color::from_rgb8(220, 231, 237)
            } else {
                Color::from_rgb8(36, 55, 70)
            },
            primary: Color::from_rgb8(39, 143, 133),
            success: Color::from_rgb8(56, 162, 128),
            danger: Color::from_rgb8(208, 83, 83),
            warning: Color::from_rgb8(217, 165, 56),
        },
    )
}

#[cfg(test)]
mod theme_render_tests {
    use super::*;
    use iced::advanced::{layout, mouse, renderer, widget::Tree, Layout, Renderer as _};

    #[test]
    fn theme_switch_draws_a_damage_tracked_full_window_background() {
        let (state, _) = State::boot();
        let size = iced::Size::new(1280.0, 820.0);
        let viewport = iced::Rectangle::with_size(size);
        let mut renderer = iced::Renderer::new(iced::Font::DEFAULT, 16.0.into());
        for dark in [false, true, false] {
            let theme = native_theme(dark);
            let mut view = state.view(window::Id::unique());
            let mut tree = Tree::new(view.as_widget());
            let layout = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, size),
            );
            renderer.reset(viewport);
            view.as_widget().draw(
                &tree,
                &mut renderer,
                &theme,
                &renderer::Style {
                    text_color: theme.palette().text,
                },
                Layout::new(&layout),
                mouse::Cursor::Unavailable,
                &viewport,
            );
            assert!(
                renderer.layers().iter().flat_map(|layer| &layer.quads).any(
                    |(quad, background)| quad.bounds == viewport
                        && *background == iced::Background::Color(theme.palette().background)
                ),
                "theme background must be a full-window damage-tracked quad; otherwise reused buffers keep the previous background"
            );
        }
    }
}
fn panel(theme: &Theme) -> container::Style {
    let dark = theme.palette().background.r < 0.5;
    container::Style {
        background: Some(
            if dark {
                Color::from_rgb8(24, 37, 49)
            } else {
                Color::WHITE
            }
            .into(),
        ),
        border: iced::Border {
            width: 1.0,
            color: if dark {
                Color::from_rgb8(42, 57, 69)
            } else {
                Color::from_rgb8(220, 228, 234)
            },
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn run() -> iced::Result {
    let args: Vec<_> = std::env::args().collect();
    #[cfg(target_os = "linux")]
    let _tray_host = if args.iter().any(|arg| arg == "--lifecycle-test") {
        tray::lifecycle_host().unwrap_or_else(|error| {
            eprintln!("Lifecycle tray host failed: {error}");
            std::process::exit(1);
        })
    } else {
        None
    };
    let autostart_test = args.iter().any(|arg| arg == "--autostart-test");
    let is_test = args.iter().any(|arg| {
        [
            "--smoke-test",
            "--runtime-test",
            "--onboarding-test",
            "--lifecycle-test",
            "--autostart-test",
        ]
        .contains(&arg.as_str())
    });
    if is_test && std::env::var_os("DRAUGR_TIME_DATA_DIR").is_none() {
        // Test modes record fixture activity; never write it into the user's history.
        let directory =
            std::env::temp_dir().join(format!("draugr-time-test-{}", std::process::id()));
        std::env::set_var("DRAUGR_TIME_DATA_DIR", directory);
    }
    if autostart_test {
        let result = startup::SystemStartup::new().and_then(|mut provider| {
            // Leave the user's real login item as it was before the test.
            let original = provider.is_enabled()?;
            let enabled = startup::set_enabled(&mut provider, true);
            let disabled = startup::set_enabled(&mut provider, false);
            let restored = startup::set_enabled(&mut provider, original);
            if !enabled? || disabled? {
                return Err("system startup state did not follow enable/disable".into());
            }
            if restored? != original {
                return Err("could not restore the previous startup setting".into());
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                TEST_PASSED.store(true, Ordering::Relaxed);
                eprintln!("Iced system startup registration passed");
                return Ok(());
            }
            Err(error) => {
                eprintln!("Iced system startup registration failed: {error}");
                std::process::exit(1);
            }
        }
    }
    let _instance_lock = match single_instance::SingleInstanceLock::acquire() {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            eprintln!("Draugr Time is already running.");
            return Ok(());
        }
        Err(error) => {
            eprintln!("Cannot check whether Draugr Time is already running: {error}");
            std::process::exit(1);
        }
    };
    if !is_test && !args.iter().any(|arg| arg == "--benchmark") {
        if let Err(error) =
            startup::SystemStartup::new().and_then(|mut startup| startup.migrate_legacy())
        {
            eprintln!("Startup registration migration failed: {error}");
        }
    }

    let result = iced::daemon(State::boot, State::update, State::view)
        .settings(iced::Settings {
            id: Some("org.draugrtime.app".into()),
            ..Default::default()
        })
        .title(|_: &State, _| "Draugr Time".to_string())
        .subscription(State::subscription)
        .theme(|state: &State, _| state.theme.clone())
        .run();
    if is_test && !TEST_PASSED.load(Ordering::Relaxed) {
        eprintln!("Iced test exited before completing its checks");
        std::process::exit(1);
    }
    result
}

#[cfg(test)]
mod load_tests {
    use super::*;

    #[test]
    fn clicking_an_application_expands_documents_and_clicking_again_collapses() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let _ = state.update(Message::Tab(1));
        let key = state.rows[0].key.clone();
        let _ = state.update(Message::Highlight(key.to_string()));
        assert!(!state.documents.is_empty());
        assert!(state
            .documents
            .windows(2)
            .all(|rows| rows[0].duration >= rows[1].duration));
        let _ = state.update(Message::Highlight(key.to_string()));
        assert!(state.documents.is_empty());
        let _ = state.update(Message::Highlight(key.to_string()));
        let _ = state.update(Message::Select(Some(Range::new(
            state.day.start,
            state.day.start + 1,
        ))));
        assert!(state.documents.is_empty());
        let _ = state.update(Message::Tab(2));
        assert!(state.documents.is_empty());
    }

    #[test]
    fn normal_launch_starts_with_no_generated_activity() {
        let (state, _) = State::boot();
        assert!(
            state.all.iter().all(|track| track.segments.is_empty()),
            "normal launches must not show synthetic demo history"
        );
    }

    #[test]
    fn stale_load_cannot_replace_a_different_day_or_finish_its_load() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        state.invalidate_load();
        let old_generation = state.load_generation;
        let old_tracks = state.all.clone();
        state.change_date(state.date.succ_opt().unwrap());
        state.loading = true;
        let _ = state.update(Message::Loaded(
            old_generation,
            Ok(Snapshot {
                tracks: old_tracks,
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(state.all.iter().all(|track| track.segments.is_empty()));
        assert!(
            state.loading,
            "an old result must not clear the new load's pending flag"
        );
        assert_eq!(state.loaded_day, None);
    }

    #[test]
    fn stale_live_load_cannot_replace_smoke_fixture() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        let old_generation = state.load_generation;
        state.fixture = true;
        state.all = model::fixture_with_apps(state.day, &state.preview_apps);
        state.invalidate_load();
        let preview = state.all.clone();
        let _ = state.update(Message::Loaded(
            old_generation,
            Ok(Snapshot {
                tracks: model::empty_tracks(),
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(state.fixture);
        assert!(
            state.all == preview,
            "a stale live result must not replace the preview"
        );
        assert_eq!(state.loaded_day, None);
    }

    #[test]
    fn current_load_updates_history_and_finishes_loading() {
        let (mut state, _) = State::boot();
        state.fixture = false;
        state.loading = true;
        let _ = state.update(Message::Loaded(
            state.load_generation,
            Ok(Snapshot {
                tracks: model::empty_tracks(),
                icons: HashMap::new(),
                ..Snapshot::default()
            }),
        ));
        assert!(!state.loading);
        assert_eq!(state.loaded_day, Some(state.day));
        assert!(state.all.iter().all(|track| track.segments.is_empty()));
    }
    #[test]
    fn incremental_snapshots_match_full_history_across_gaps_and_search_changes() {
        use draugr_time_core::{Collection, Storage};
        use draugr_time_daemon::Shared;
        use serde_json::json;
        let shared = Shared::new(Storage::open_in_memory().unwrap(), "test".into(), 300);
        let (mut state, _) = State::boot();
        state.day = Range::new(0, 100_000);
        state.all = model::empty_tracks();
        for (now, app) in [
            (1000, "alpha"),
            (2000, "alpha"),
            (10_000, "beta"),
            (11_000, "beta"),
            (12_000, "gamma"),
            (13_000, "gamma"),
        ] {
            {
                let mut storage = shared.storage.lock().unwrap();
                storage
                    .heartbeat("window", &json!({"app": app, "title": app}), now, 3000)
                    .unwrap();
                storage
                    .heartbeat("afk", &json!({"status":"active"}), now, 3000)
                    .unwrap();
            }
            let snapshot =
                Runtime::load(shared.clone(), state.day, state.load_cursor.clone()).unwrap();
            let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
            let storage = shared.storage.lock().unwrap();
            let expected = model::tracks(
                &storage.events(None, 0, 100_000).unwrap(),
                &[],
                &[],
                state.day,
            );
            assert_eq!(state.all, expected, "incremental result at {now}");
        }
        let _ = state.update(Message::Search("GAMMA".into()));
        assert_eq!(state.filtered[2].len(), 1);
        let _ = state.update(Message::Search("alpha".into()));
        assert_eq!(state.filtered[2].len(), 1);
        {
            let mut storage = shared.storage.lock().unwrap();
            let tag = storage
                .put(Collection::Tags, None, json!({"name":"Project"}))
                .unwrap();
            storage
                .put(
                    Collection::TagRanges,
                    None,
                    json!({"tag_id":tag["id"],"start_ts":1000,"end_ts":2000}),
                )
                .unwrap();
        }
        let snapshot = Runtime::load(shared.clone(), state.day, state.load_cursor.clone()).unwrap();
        let _ = state.update(Message::Loaded(state.load_generation, Ok(snapshot)));
        let storage = shared.storage.lock().unwrap();
        assert_eq!(
            state.all,
            model::tracks(
                &storage.events(None, 0, 100_000).unwrap(),
                &storage.tags().unwrap(),
                &storage.tag_ranges(state.day.start, state.day.end).unwrap(),
                state.day
            )
        );
    }

    #[test]
    fn segment_strings_are_shared_between_application_and_document_tracks() {
        let tracks = model::fixture(Range::new(0, 86_400_000));
        let app = &tracks[2].segments[0];
        let document = &tracks[3].segments[0];
        assert!(Arc::ptr_eq(
            app.app.as_ref().unwrap(),
            document.app.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(&app.detail, &document.label));
        let _ =
            model::application_documents(&tracks[3].segments, &app.key, Range::new(0, 86_400_000));
    }
}

#[cfg(test)]
mod tagging_tests;

// macOS samples on the GUI thread and always ticks, so only other platforms can idle.
#[cfg(all(test, not(target_os = "macos")))]
mod tick_tests {
    use super::*;

    fn recording_state() -> State {
        let (mut state, _) = State::boot();
        state.window = Some(window::Id::unique());
        state.fixture = false;
        state.runtime = Some(Runtime::for_test(draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        )));
        state
    }

    #[test]
    fn open_window_ticks_until_the_shown_day_is_loaded() {
        let mut state = recording_state();
        state.loaded_day = None;
        assert!(state.tick_demand().needed());
    }

    #[test]
    fn live_day_ticks_only_on_the_day_screen() {
        let mut state = recording_state();
        state.loaded_day = Some(state.day);
        assert!(
            state.tick_demand().needed(),
            "today keeps refreshing on Day"
        );
        state.set_screen("Settings");
        assert!(
            !state.tick_demand().needed(),
            "Settings shows no live history"
        );
        state.set_screen("Day");
        assert!(state.tick_demand().needed());
    }

    #[test]
    fn loaded_past_day_and_closed_window_stop_ticking() {
        let mut state = recording_state();
        let yesterday = state.date.checked_sub_days(Days::new(1)).unwrap();
        state.change_date(yesterday);
        assert!(
            state.tick_demand().needed(),
            "a new day still needs its first load"
        );
        state.loaded_day = Some(state.day);
        assert!(
            !state.tick_demand().needed(),
            "past history does not change"
        );
        state.change_date(Local::now().date_naive());
        state.loaded_day = Some(state.day);
        state.window = None;
        assert!(!state.tick_demand().needed());
    }

    #[test]
    fn preview_without_runtime_does_not_tick() {
        let mut state = recording_state();
        state.runtime = None;
        state.loaded_day = None;
        assert!(!state.tick_demand().needed());
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    #[test]
    fn closing_releases_day_and_invalidates_pending_load_without_losing_edits() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.search = "window".into();
        state.rebuild();
        state.loaded_day = Some(state.day);
        state.loading = true;
        state.editor = Some(tagging::Editor {
            name: "Unsaved".into(),
            ..Default::default()
        });
        let mut storage = draugr_time_core::Storage::open_in_memory().unwrap();
        state
            .tag_undo
            .push(storage.tag_interval("Kept undo", "", 1, 2, false).unwrap());
        let generation = state.load_generation;
        state.release_day();
        assert!(state.all.iter().all(|t| t.segments.capacity() == 0));
        assert!(state.filtered.iter().all(|indices| indices.capacity() == 0));
        assert!(state.search_text.is_empty());
        assert_eq!(state.rows.capacity(), 0);
        assert_eq!(state.documents.capacity(), 0);
        assert_eq!(state.icons.capacity(), 0);
        assert_eq!(state.tags.capacity(), 0);
        assert_eq!(state.tag_ranges.capacity(), 0);
        assert_eq!(state.loaded_day, None);
        assert!(state.load_cursor.starts.is_empty());
        assert!(!state.loading);
        assert_eq!(state.editor.as_ref().unwrap().name, "Unsaved");
        assert_eq!(state.tag_undo.len(), 1);
        let _ = state.update(Message::Loaded(
            generation,
            Ok(Snapshot {
                tracks: model::fixture(state.day),
                ..Default::default()
            }),
        ));
        assert!(state.all.iter().all(|t| t.segments.is_empty()));
        let _ = state.update(Message::Loaded(
            state.load_generation,
            Ok(Snapshot {
                tracks: model::fixture(state.day),
                ..Default::default()
            }),
        ));
        assert!(!state.all[2].segments.is_empty());
    }
    #[test]
    fn search_cache_is_lazy_shared_and_discarded() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        assert!(state.search_text.is_empty());
        let _ = state.update(Message::Search("window".into()));
        assert!(!state.search_text.is_empty());
        let count = state.search_text.len();
        assert!(
            count
                < state
                    .all
                    .iter()
                    .map(|track| track.segments.len() * 3)
                    .sum::<usize>()
        );
        assert!(state
            .search_text
            .values()
            .any(|(text, _)| Arc::strong_count(text) > 2));
        let _ = state.update(Message::Search("WINDOW".into()));
        assert_eq!(state.search_text.len(), count);
        let _ = state.update(Message::Search(String::new()));
        assert!(state.search_text.is_empty());
    }
    #[test]
    fn raw_icons_preserve_format_and_skip_missing_images() {
        let icons = decode_icons(
            [
                (
                    "png".into(),
                    Some(crate::icons::IconData::Png(vec![1, 2, 3])),
                ),
                (
                    "svg".into(),
                    Some(crate::icons::IconData::Svg(b"<svg/>".to_vec())),
                ),
                ("missing".into(), None),
            ]
            .into(),
        );
        assert!(matches!(icons.get("png"), Some(AppIcon::Png(_))));
        assert!(matches!(icons.get("svg"), Some(AppIcon::Svg(_))));
        assert!(!icons.contains_key("missing"));
    }
    #[test]
    fn replacing_tail_updates_search_results() {
        let (mut state, _) = State::boot();
        state.day = Range::new(0, 100);
        state.all = model::tracks(
            &[draugr_time_core::Event {
                id: 1,
                bucket: "window".into(),
                start_ts: 1,
                end_ts: 10,
                data: serde_json::json!({"app":"old", "title":"old"}),
            }],
            &[],
            &[],
            state.day,
        );
        let _ = state.update(Message::Search("new".into()));
        assert!(state.filtered[2].is_empty());
        let incoming = model::tracks(
            &[draugr_time_core::Event {
                id: 1,
                bucket: "window".into(),
                start_ts: 1,
                end_ts: 20,
                data: serde_json::json!({"app":"new", "title":"new"}),
            }],
            &[],
            &[],
            state.day,
        );
        let _ = state.update(Message::Loaded(
            state.load_generation,
            Ok(Snapshot {
                tracks: incoming,
                replace_from: [("window".into(), 1)].into(),
                ..Default::default()
            }),
        ));
        assert_eq!(state.filtered[2].len(), 1);
        let _ = state.update(Message::Search("old".into()));
        assert!(state.filtered[2].is_empty());
    }
}

#[cfg(test)]
mod panel_tests {
    use super::*;
    use iced::advanced::{layout, mouse, widget::Tree, Layout};
    fn split(state: &State) -> (widget::pane_grid::Split, f32) {
        match state.day_panes.layout() {
            widget::pane_grid::Node::Split { id, ratio, .. } => (*id, *ratio),
            _ => panic!("Day must have two vertically stacked panes"),
        }
    }
    #[test]
    fn resizing_preserves_selection_zoom_data_and_reopen_ratio() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        state.selection = Some(Range::new(state.day.start, state.day.start + 60_000));
        state.view = Range::new(state.day.start, state.day.start + 3_600_000);
        let old = (state.selection, state.view, state.all.clone());
        let (id, _) = split(&state);
        let _ = state.update(Message::ResizeDay(widget::pane_grid::ResizeEvent {
            split: id,
            ratio: 0.75,
        }));
        assert_eq!(split(&state).1, 0.75);
        assert_eq!((state.selection, state.view, state.all.clone()), old);
        state.release_day();
        state.window = None;
        let _ = state.open(true);
        assert_eq!(split(&state).1, 0.75);
        let _ = state.update(Message::Key(
            keyboard::Key::Named(keyboard::key::Named::ArrowUp),
            keyboard::Modifiers::ALT,
        ));
        assert!((split(&state).1 - 0.70).abs() < 0.001);
    }
    #[test]
    fn extreme_ratios_keep_panes_inside_available_space() {
        let (mut state, _) = State::boot();
        let (id, before) = split(&state);
        let _ = state.update(Message::ResizeDay(widget::pane_grid::ResizeEvent {
            split: id,
            ratio: f32::NAN,
        }));
        assert_eq!(split(&state).1, before);
        for ratio in [-1.0, 0.0, 0.5, 1.0, 2.0] {
            let _ = state.update(Message::ResizeDay(widget::pane_grid::ResizeEvent {
                split: id,
                ratio,
            }));
            let size = iced::Size::new(1000.0, 500.0);
            let regions = state.day_panes.layout().pane_regions(10.0, 160.0, size);
            assert_eq!(regions.len(), 2);
            for region in regions.values() {
                assert!(region.height >= 160.0);
                assert!(region.y >= 0.0 && region.y + region.height <= size.height);
            }
        }
    }
    #[test]
    fn native_divider_drag_emits_resize_without_selecting_activity() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let mut view = state.day_view();
        let renderer = iced::Renderer::new(iced::Font::DEFAULT, 16.0.into());
        let size = iced::Size::new(1000.0, 700.0);
        let viewport = iced::Rectangle::with_size(size);
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(iced::Size::ZERO, size),
        );
        let layout = Layout::new(&node);
        let bounds = layout.children().last().unwrap().bounds();
        let (axis, region, ratio) = state
            .day_panes
            .layout()
            .split_regions(10.0, 160.0, bounds.size())
            .into_values()
            .next()
            .unwrap();
        let line = axis.split_line_bounds(region, ratio, 10.0);
        let start = iced::Point::new(bounds.x + line.center_x(), bounds.y + line.center_y());
        let end = iced::Point::new(start.x, start.y + 60.0);
        assert_eq!(
            view.as_widget().mouse_interaction(
                &tree,
                layout,
                mouse::Cursor::Available(start),
                &viewport,
                &renderer
            ),
            mouse::Interaction::ResizingVertically
        );
        let mut messages = Vec::new();
        for (event, point) in [
            (
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                start,
            ),
            (
                iced::Event::Mouse(mouse::Event::CursorMoved { position: end }),
                end,
            ),
            (
                iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                end,
            ),
        ] {
            view.as_widget_mut().update(
                &mut tree,
                &event,
                layout,
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut iced::advanced::Shell::new(&mut messages),
                &viewport,
            );
        }
        assert!(messages
            .iter()
            .any(|message| matches!(message, Message::ResizeDay(event) if event.ratio > ratio)));
        assert!(!messages
            .iter()
            .any(|message| matches!(message, Message::Select(_))));
    }
}

#[cfg(test)]
mod localization_tests;

#[cfg(test)]
mod about_tests {
    use super::open_command;

    #[test]
    fn open_command_passes_the_url_to_the_platform_launcher() {
        let (program, args) = open_command("https://github.com/hlophlopgaming/DraugrTime");
        assert!(!program.is_empty());
        assert!(
            args.iter()
                .any(|arg| arg == "https://github.com/hlophlopgaming/DraugrTime"),
            "{args:?}"
        );
    }
}

#[cfg(test)]
mod overview_icon_tests {
    #[test]
    fn overview_name_uses_the_icon_cached_under_process_and_name() {
        let icons =
            std::collections::HashMap::from([("/usr/bin/gwenview\0org.kde.gwenview".into(), 1)]);
        assert_eq!(
            super::cached_app_icon(&icons, "org.kde.gwenview", "org.kde.gwenview"),
            Some(&1)
        );
        assert_eq!(
            super::cached_app_icon(&icons, "/other", "org.kde.gwenview"),
            None
        );
    }
}
