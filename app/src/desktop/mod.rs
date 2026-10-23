//! Native Draugr Time desktop.
mod appearance;
mod components;
mod day;
mod design;
mod localization;
mod management;
mod reports;
#[cfg(test)]
mod reports_tests;
mod rules;
mod settings;
#[cfg(test)]
mod settings_tests;
mod setup;
use design::{button, text_input};
use localization::{message as localized, tr, LocaleProvider};
mod memory;
mod model;
mod motion;
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
        self, checkbox, column, container, image, row, scrollable, space, svg, text, tooltip,
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
    Manage(management::Edit),
    Report(reports::Edit),
    Language(Option<localization::Language>),
    Tick,
    MotionFrame(Instant),
    MotionChanged(bool),
    MotionRefresh,
    MotionUnfocused,
    Tray(tray::Event),
    Opened(window::Id),
    Close(window::Id),
    Closed,
    Screen(&'static str),
    Shift(i32),
    Today,
    Search(String),
    Tab(usize),
    Select(Option<Range>),
    SelectDragging(Option<Range>),
    SelectionReleased,
    Hover(Option<(usize, usize)>),
    Highlight(String),
    Zoom(f64, Option<i64>),
    PanBy(f64),
    DayRange(usize),
    ExpandTrack(usize),
    ReviewGaps,
    TagGap(Range),
    SelectionTag(String),
    ApplySelectionTag,
    ExcludeSelection,
    SummaryFilter(String),
    ExpandSummary(String),
    RecMenu,
    ToastExpired(Instant),
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
struct State {
    settings_ui: settings::Ui,
    report_ui: reports::Ui,
    rule_editor: rules::Editor,
    management: management::Ui,
    language_choice: Option<localization::Language>,
    tags: Vec<draugr_time_core::Tag>,
    tag_ranges: Vec<draugr_time_core::TagRange>,
    recent_tags: Vec<i64>,
    editor: Option<tagging::Editor>,
    tag_undo: Vec<draugr_time_core::TagUndo>,
    away_prompt: tagging::AwayPrompt,
    away_range: Option<Range>,
    date: NaiveDate,
    day: Range,
    view: Range,
    all: Vec<Track>,
    filtered: Vec<Vec<usize>>,
    search_text: HashMap<usize, (Arc<str>, String)>,
    order: Vec<usize>,
    visible: Vec<usize>,
    shown: [bool; 4],
    selection: Option<Range>,
    selecting: bool,
    hover: Option<(usize, usize)>,
    highlight: Option<String>,
    expanded_summary: Option<String>,
    summary_closing: Option<String>,
    summary_search: String,
    selection_tag: String,
    search: String,
    screen: &'static str,
    rail_focus: Option<usize>,
    tab: usize,
    rows: Vec<Summary>,
    documents: Vec<Summary>,
    limit: usize,
    metrics: [i64; 4],
    fixture: bool,
    expanded_tracks: [bool; 4],
    motion: motion::Ui,
    motion_visible: bool,
    highlight_from: Vec<(Option<String>, f32)>,
    rec_menu: bool,
    search_open: bool,
    block_focus: bool,
    tag_toast: Option<(Range, Instant)>,
    range_mode: usize,
    theme: Theme,
    theme_preference: appearance::Preference,
    system_theme: iced::theme::Mode,
    grid_cache: widget::canvas::Cache,
    hourly_cache: widget::canvas::Cache,
    hourly: [[i64; 3]; 24],
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
            management: management::Ui::default(),
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
            selecting: false,
            hover: None,
            highlight: None,
            expanded_summary: None,
            summary_closing: None,
            summary_search: String::new(),
            selection_tag: String::new(),
            documents: Vec::new(),
            search: String::new(),
            screen: "Day",
            rail_focus: None,
            tab: 1,
            rows: Vec::new(),
            limit: 50,
            metrics: [0; 4],
            fixture: smoke || benchmark,
            expanded_tracks: [false; 4],
            motion: motion::Ui::default(),
            motion_visible: false,
            highlight_from: Vec::new(),
            rec_menu: false,
            search_open: false,
            block_focus: false,
            tag_toast: None,
            range_mode: 1,
            theme: native_theme(benchmark && args.iter().any(|arg| arg == "--dark")),
            theme_preference: if benchmark && args.iter().any(|arg| arg == "--dark") {
                appearance::Preference::Dark
            } else if benchmark && args.iter().any(|arg| arg == "--light") {
                appearance::Preference::Light
            } else {
                appearance::Preference::System
            },
            system_theme: iced::theme::Mode::None,
            grid_cache: widget::canvas::Cache::new(),
            hourly_cache: widget::canvas::Cache::new(),
            hourly: [[0; 3]; 24],
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
        if self.screen != screen {
            self.finish_motion();
        }
        if matches!(self.screen, "Week" | "Reports") && self.screen != screen {
            self.release_reports();
        }
        if matches!(self.screen, "Tags" | "Rules") && !matches!(screen, "Tags" | "Rules") {
            self.release_management();
        }
        self.screen = screen;
    }
    fn apply_theme(&mut self) {
        self.finish_motion();
        self.hourly_cache.clear();
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
            size: iced::Size::new(
                if self.benchmark && std::env::args().any(|arg| arg == "--narrow") {
                    640.0
                } else {
                    1280.0
                },
                820.0,
            ),
            min_size: Some(iced::Size::new(480.0, 600.0)),
            visible,
            exit_on_close_request: false,
            icon: crate::branding::window_icon().ok(),
            ..window::Settings::default()
        });
        self.window = Some(id);
        self.motion_visible = visible;
        let task = task.map(Message::Opened);
        if matches!(self.screen, "Tags" | "Rules") {
            Task::batch([task, self.refresh_management()])
        } else {
            task
        }
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
        self.selecting = false;
        self.motion.clear();
        self.highlight_from.clear();
        self.release_management();
        self.hourly_cache.clear();
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
        self.hourly = [[0; 3]; 24];
        self.hover = None;
        self.cache.clear();
        self.grid_cache.clear();
    }
    fn rebuild(&mut self) {
        self.hourly_cache.clear();
        self.hourly = day::hourly_totals(&self.all, self.day);
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
        self.metrics[3] = self.day_gaps().iter().map(|r| r.duration()).sum();
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
        if self.range_mode == 1 {
            self.view = self.active_hours().unwrap_or(self.day);
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
            self.day,
        );
        let observed = self.rows.iter().map(|r| r.duration).sum::<i64>().max(1);
        for row in &mut self.rows {
            row.share = row.duration as f64 / observed as f64 * 100.0;
        }
        let search = self.summary_search.to_lowercase();
        if !search.trim().is_empty() {
            let matches: std::collections::HashSet<_> = self.all[3]
                .segments
                .iter()
                .filter(|s| {
                    s.label.to_lowercase().contains(&search)
                        || s.detail.to_lowercase().contains(&search)
                })
                .map(|s| {
                    s.app.as_ref().map(|app| {
                        model::application_identity(app, s.app_name.as_deref().unwrap_or(app))
                    })
                })
                .collect();
            self.rows.retain(|row| {
                row.label.to_lowercase().contains(&search)
                    || matches.contains(&Some(row.key.to_string()))
            });
        }
        self.documents = if self.tab == 1 {
            self.expanded_summary
                .as_ref()
                .or(self
                    .summary_closing
                    .as_ref()
                    .filter(|_| self.motion.active(motion::Key::Summary)))
                .or(self.highlight.as_ref())
                .map(|key| {
                    model::application_documents_iter(
                        self.filtered[3].iter().map(|i| &self.all[3].segments[*i]),
                        key,
                        self.day,
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
        self.day = model::day_bounds(date);
        self.range_mode = 1;
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
        self.motion.clear();
        self.rebuild();
        if self.fixture {
            self.animate(motion::Key::Date, 0.0, 1.0);
        }
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
                        || key == keyboard::Key::Named(keyboard::key::Named::F6)
                        || (modifiers.command()
                            && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("f"))) =>
                {
                    Some(Message::Key(key, modifiers))
                }
                _ => None,
            }),
        ];
        if self
            .motion
            .running(self.window.is_some() && self.motion_visible)
        {
            subscriptions
                .push(iced::time::every(Duration::from_millis(16)).map(Message::MotionFrame));
        }
        if self.window.is_some() {
            subscriptions.push(window::events().filter_map(|(_, event)| match event {
                window::Event::Focused => Some(Message::MotionRefresh),
                window::Event::Unfocused => Some(Message::MotionUnfocused),
                _ => None,
            }));
            use motion::MotionPreference as _;
            subscriptions.push(
                motion::SystemMotionPreference
                    .changes()
                    .map(Message::MotionChanged),
            );
        }
        if self.management.dragging.is_some() {
            subscriptions.push(iced::event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::Manage(management::Edit::EndDrag)),
                _ => None,
            }));
        }
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
            Message::MotionFrame(now) => {
                if self.motion.active(motion::Key::Track(2))
                    || self.motion.active(motion::Key::Track(3))
                {
                    self.cache.clear();
                    self.grid_cache.clear();
                }
                let summary = self.motion.active(motion::Key::Summary);
                self.motion.advance(now);
                if !self.motion.active(motion::Key::Highlight) {
                    self.highlight_from.clear();
                }
                if summary && !self.motion.active(motion::Key::Summary) {
                    self.summary_closing = None;
                    self.summary();
                }
            }
            Message::MotionChanged(reduced) => {
                if reduced {
                    self.finish_motion();
                }
                self.motion.set_reduced(reduced);
            }
            Message::MotionRefresh => {
                self.motion_visible = true;
                return motion::read_preference();
            }
            Message::MotionUnfocused => {
                self.motion_visible = false;
                self.finish_motion();
            }
            Message::Report(event) => return self.edit_report(event),
            Message::Edit(event) => {
                self.selecting = false;
                return self.edit(event);
            }
            Message::Manage(event) => return self.edit_management(event),
            Message::Rule(event) => {
                let refresh = matches!(
                    event,
                    rules::Edit::Save | rules::Edit::Delete | rules::Edit::Confirm
                );
                self.edit_rule(event);
                if refresh {
                    return self.refresh_management();
                }
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
                    id if tray::pause_duration(id).is_some() => {
                        Message::Pause(tray::pause_duration(id))
                    }
                    "extend5" | "extend15" if self.status.state == "paused" => {
                        let minutes = if event == "extend5" { 5 } else { 15 };
                        let remaining = self
                            .status
                            .paused_until
                            .unwrap_or(0)
                            .saturating_sub(draugr_time_daemon::now_ms())
                            .max(0);
                        Message::Pause(Some(remaining + minutes * 60_000))
                    }
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
                if (self.status.state == "paused") != (status.state == "paused") {
                    self.animate(
                        motion::Key::Pause,
                        f32::from(self.status.state == "paused"),
                        f32::from(status.state == "paused"),
                    );
                }
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
                    self.finish_motion();
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
            Message::Opened(id) => {
                self.window = Some(id);
                if self.autostart_question() {
                    self.animate(motion::Key::Dialog, 0.0, 1.0);
                }
                return motion::read_preference();
            }
            Message::Close(id) => {
                if self.tray.is_some() {
                    self.window = None;
                    self.release_day();
                    return window::close(id);
                }
                return iced::exit();
            }
            Message::Closed => memory::SystemReclaimer.reclaim(),
            Message::Screen(screen) => {
                self.set_screen(screen);
                if matches!(screen, "Tags" | "Rules") {
                    return self.refresh_management();
                }
                if matches!(screen, "Overview" | "Week" | "Reports") {
                    return self.edit_report(reports::Edit::Refresh);
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
                if self.tab != tab {
                    self.animate(
                        motion::Key::Tab,
                        self.tab_position(),
                        match tab {
                            1 => 0.0,
                            2 => 1.0,
                            _ => 2.0,
                        },
                    );
                }
                self.tab = tab;
                self.highlight = None;
                self.limit = 50;
                self.cache.clear();
                self.grid_cache.clear();
                self.summary();
            }
            Message::SelectDragging(selection) => {
                self.selecting = true;
                self.selection = selection;
                self.highlight = None;
                self.summary();
            }
            Message::SelectionReleased => {
                if self.selecting {
                    self.selecting = false;
                    self.motion.cancel(motion::Key::Selection);
                    self.animate(motion::Key::Selection, 0.0, 1.0);
                }
            }
            Message::Select(selection) => {
                self.selecting = false;
                if selection != self.selection {
                    self.animate(motion::Key::Selection, 0.0, 1.0);
                }
                self.selection = selection;
                self.highlight = None;
                self.limit = 50;
                self.summary();
            }
            Message::Hover(hover) => {
                self.block_focus = false;
                if self.hover != hover {
                    self.motion.cancel(motion::Key::Tooltip);
                    if hover.is_some() {
                        self.animate(motion::Key::Tooltip, 0.0, 1.0);
                    }
                }
                self.hover = hover;
                if hover.is_some() && self.motion.active(motion::Key::Tooltip) {
                    return Task::perform(
                        async {
                            tokio::time::sleep(Duration::from_millis(design::TOOLTIP_DELAY)).await;
                            Instant::now()
                        },
                        Message::MotionFrame,
                    );
                }
            }
            Message::Highlight(key) => {
                motion::highlight_snapshot(
                    &mut self.highlight_from,
                    self.highlight.as_deref(),
                    self.motion.value(motion::Key::Highlight, 1.0),
                );
                self.motion.cancel(motion::Key::Highlight);
                self.animate(motion::Key::Highlight, 0.0, 1.0);
                self.highlight = if self.highlight.as_ref() == Some(&key) {
                    None
                } else {
                    Some(key)
                };
                self.summary();
            }
            Message::Zoom(factor, anchor) => {
                self.range_mode = 3;
                let anchor = anchor.or_else(|| self.selection.map(|s| s.start + s.duration() / 2));
                self.view = model::zoom(self.day, self.view, factor, anchor);
                self.hover = None;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::RecMenu => {
                self.animate(
                    motion::Key::Menu,
                    f32::from(self.rec_menu),
                    f32::from(!self.rec_menu),
                );
                self.rec_menu = !self.rec_menu;
            }
            Message::ToastExpired(started) => {
                if self.tag_toast.is_some_and(|(_, time)| time == started) {
                    self.tag_toast = None;
                }
            }
            Message::ExpandTrack(track) => {
                if (2..=3).contains(&track) {
                    self.animate(
                        motion::Key::Track(track),
                        f32::from(self.expanded_tracks[track]),
                        f32::from(!self.expanded_tracks[track]),
                    );
                    self.expanded_tracks[track] = !self.expanded_tracks[track];
                    self.cache.clear();
                    self.grid_cache.clear();
                }
            }
            Message::SelectionTag(name) => self.selection_tag = name,
            Message::ApplySelectionTag => {
                let task = self.edit(tagging::Edit::Open);
                if self.selection_tag.is_empty() {
                    return task;
                }
                let _ = self.edit(tagging::Edit::Name(self.selection_tag.clone()));
                return self.edit(tagging::Edit::Save(false));
            }
            Message::ExcludeSelection => {
                self.set_screen("Settings");
                return self.edit_settings(settings::Edit::Delete);
            }
            Message::TagGap(range) => {
                let _ = self.update(Message::Select(Some(range)));
                return self.edit(tagging::Edit::Open);
            }
            Message::SummaryFilter(value) => {
                self.summary_search = value;
                self.summary();
            }
            Message::ExpandSummary(key) => {
                let opening = self.expanded_summary.as_ref() != Some(&key);
                self.summary_closing = if opening {
                    None
                } else {
                    self.expanded_summary.clone()
                };
                self.animate(
                    motion::Key::Summary,
                    f32::from(!opening),
                    f32::from(opening),
                );
                self.expanded_summary = if self.expanded_summary.as_ref() == Some(&key) {
                    None
                } else {
                    Some(key)
                };
                self.summary();
            }
            Message::ReviewGaps => {
                let gaps = self.day_gaps();
                if let Some(range) = gaps
                    .iter()
                    .find(|r| self.selection.is_none_or(|s| r.start >= s.end))
                    .or(gaps.first())
                    .copied()
                {
                    return self.update(Message::Select(Some(range)));
                }
            }
            Message::DayRange(mode) => {
                if self.range_mode != mode {
                    self.animate(
                        motion::Key::Range,
                        self.range_mode.min(2) as f32,
                        mode as f32,
                    );
                }
                self.range_mode = mode;
                self.view = match mode {
                    2 => self.selection.unwrap_or(self.view),
                    1 => self.active_hours().unwrap_or(self.day),
                    _ => self.day,
                };
                self.hover = None;
                self.cache.clear();
                self.grid_cache.clear();
            }
            Message::PanBy(value) => {
                self.range_mode = 3;
                let span = self.view.duration();
                let start = (self.view.start + (value * span as f64) as i64)
                    .clamp(self.day.start, self.day.end - span);
                self.view = Range::new(start, start + span);
                self.cache.clear();
                self.grid_cache.clear();
                self.hover = None;
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
            Message::Start => {
                self.start();
                if matches!(self.screen, "Tags" | "Rules") {
                    return self.refresh_management();
                }
            }
            Message::AutoStart(enabled) => match startup::SystemStartup::new()
                .and_then(|mut provider| startup::set_enabled(&mut provider, enabled))
            {
                Ok(enabled) => {
                    self.autostart_enabled = enabled;
                    self.autostart_error.clear();
                    if self.settings_ui.preferences.autostart_prompt_pending {
                        if let Err(error) = draugr_time_daemon::paths::data_dir()
                            .map_err(|error| error.to_string())
                            .and_then(|dir| {
                                self.settings_ui
                                    .answer_autostart(&dir.join("settings.json"))
                            })
                        {
                            self.autostart_error = error;
                        }
                    }
                }
                Err(error) => self.autostart_error = error,
            },
            Message::Pause(duration) => {
                if self.rec_menu {
                    self.animate(motion::Key::Menu, 1.0, 0.0);
                }
                self.animate(
                    motion::Key::Pause,
                    f32::from(self.status.state == "paused"),
                    1.0,
                );
                tray::remember_pause(duration);
                self.rec_menu = false;
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
                if self.rec_menu {
                    self.animate(motion::Key::Menu, 1.0, 0.0);
                }
                self.animate(
                    motion::Key::Pause,
                    f32::from(self.status.state == "paused"),
                    0.0,
                );
                self.rec_menu = false;
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
                use keyboard::key::Named;
                if self.autostart_question() || self.motion.active(motion::Key::Dialog) {
                    return if key == keyboard::Key::Named(Named::Tab) {
                        if modifiers.shift() {
                            widget::operation::focus_previous()
                        } else {
                            widget::operation::focus_next()
                        }
                    } else {
                        Task::none()
                    };
                }
                if self.rec_menu && key == keyboard::Key::Named(Named::Escape) {
                    return self.update(Message::RecMenu);
                }
                if self.screen == "Day"
                    && self.editor.is_none()
                    && self.rail_focus.is_none()
                    && modifiers.alt()
                    && matches!(key, keyboard::Key::Named(Named::ArrowDown | Named::ArrowUp))
                {
                    let blocks: Vec<_> = self
                        .visible
                        .iter()
                        .flat_map(|&t| self.filtered[t].iter().map(move |&i| (t, i)))
                        .collect();
                    if !blocks.is_empty() {
                        let old = self.hover.and_then(|h| blocks.iter().position(|b| *b == h));
                        let next = if key == keyboard::Key::Named(Named::ArrowUp) {
                            old.map(|i| (i + blocks.len() - 1) % blocks.len())
                                .unwrap_or(blocks.len() - 1)
                        } else {
                            old.map(|i| (i + 1) % blocks.len()).unwrap_or(0)
                        };
                        let (track, index) = blocks[next];
                        let _ = self
                            .update(Message::Select(Some(self.all[track].segments[index].range)));
                        self.hover = Some((track, index));
                        self.block_focus = true;
                    }
                    return Task::none();
                }

                if key == keyboard::Key::Named(Named::Tab) && self.rail_focus.is_none() {
                    return if modifiers.shift() {
                        widget::operation::focus_previous()
                    } else {
                        widget::operation::focus_next()
                    };
                }
                if self.editor.is_none() && key == keyboard::Key::Named(Named::F6) {
                    self.rail_focus = if self.rail_focus.is_some() {
                        None
                    } else {
                        Some(0)
                    };
                    return widget::operation::focus(if self.rail_focus.is_some() {
                        "rail"
                    } else {
                        "activity-search"
                    });
                }
                if self.editor.is_none() {
                    if let Some(index) = self.rail_focus {
                        match key {
                            keyboard::Key::Named(Named::ArrowDown | Named::ArrowRight) => {
                                self.rail_focus = Some((index + 1) % 6);
                                return Task::none();
                            }
                            keyboard::Key::Named(Named::ArrowUp | Named::ArrowLeft) => {
                                self.rail_focus = Some((index + 5) % 6);
                                return Task::none();
                            }
                            keyboard::Key::Named(Named::Enter | Named::Space) => {
                                return self.update(if index == 5 {
                                    Message::RecMenu
                                } else {
                                    Message::Screen(
                                        ["Day", "Week", "Tags", "Rules", "Settings"][index],
                                    )
                                });
                            }
                            keyboard::Key::Named(Named::Escape | Named::Tab) => {
                                self.rail_focus = None;
                                self.search_open = true;
                                return widget::operation::focus(if self.screen == "Day" {
                                    "day-filter"
                                } else {
                                    "activity-search"
                                });
                            }
                            _ => {}
                        }
                    }
                }
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
                if modifiers.command()
                    && matches!(&key,keyboard::Key::Character(c)if c.eq_ignore_ascii_case("z"))
                {
                    let task = self.edit(tagging::Edit::Undo);
                    if matches!(self.screen, "Tags" | "Rules") {
                        return Task::batch([task, self.refresh_management()]);
                    }
                    return task;
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
                    self.search_open = true;
                    return widget::operation::focus(if self.screen == "Day" {
                        "day-filter"
                    } else if self.screen == "Tags" {
                        "manager-tag-search"
                    } else {
                        "activity-search"
                    });
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
            Some(AppIcon::Png(handle)) => image(handle.clone())
                .width(design::APP_ICON)
                .height(design::APP_ICON)
                .into(),
            Some(AppIcon::Svg(handle)) => svg(handle.clone())
                .width(design::APP_ICON)
                .height(design::APP_ICON)
                .into(),
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
                .width(design::APP_ICON)
                .height(design::APP_ICON)
                .center(design::APP_ICON)
                .style(move |_: &Theme| container::Style {
                    background: Some(Color::from_rgb(rgb[0], rgb[1], rgb[2]).into()),
                    text_color: Some(design::Palette::new(true).accent_ink),
                    border: iced::Border::default().rounded(4),
                    ..container::Style::default()
                })
                .into()
            }
        }
    }
    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        widget::responsive(move |size| self.view_sized(size.width)).into()
    }
    fn view_sized(&self, width: f32) -> Element<'_, Message> {
        if !self.settings_ui.preferences.onboarded
            && !self.fixture
            && (!self.runtime_test || self.onboarding_test)
        {
            return self.onboarding();
        }
        if self.autostart_question() || self.motion.active(motion::Key::Dialog) {
            let p = design::Palette::new(self.theme.palette().background.r < 0.5);
            return container(motion::effect(
                self.autostart_dialog(),
                motion::Visual {
                    opacity: self
                        .motion
                        .value(motion::Key::Dialog, f32::from(self.autostart_question())),
                    scale: 0.98
                        + 0.02
                            * self
                                .motion
                                .value(motion::Key::Dialog, f32::from(self.autostart_question())),
                    ..Default::default()
                },
                p.bg,
            ))
            .padding(design::PADDING)
            .center(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.bg.into()),
                text_color: Some(p.text),
                ..Default::default()
            })
            .into();
        }
        let narrow = design::narrow(width);
        let navigation = design::navigation_metrics(localization::current());
        let rail_item = navigation.item_width;
        let rail_width = design::RAIL.max(rail_item + 2.0 * design::RAIL_INSET as f32);
        let palette = design::Palette::new(self.theme.palette().background.r < 0.5);
        let logo: Element<'_, Message> = image(image::Handle::from_bytes(crate::branding::logo(
            self.theme.palette().background.r < 0.5,
        )))
        .width(design::LOGO)
        .height(design::LOGO)
        .into();
        let mut items = vec![container(logo)
            .center_x(rail_item)
            .center_y(design::RAIL_ITEM)
            .into()];
        for (index, screen) in ["Day", "Week", "Tags", "Rules", "Settings"]
            .into_iter()
            .enumerate()
        {
            items.push(
                button(
                    column![
                        design::rail_icon(
                            screen,
                            if screen == self.screen {
                                palette.accent_text
                            } else {
                                palette.text2
                            }
                        ),
                        text(tr(screen))
                            .size(design::LABEL)
                            .width(Length::Fill)
                            .align_x(Alignment::Center)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .shaping(iced::widget::text::Shaping::Advanced)
                    ]
                    .align_x(Alignment::Center)
                    .spacing(design::SMALL_GAP),
                )
                .on_press(Message::Screen(screen))
                .padding(design::CONTROL_GAP as u16)
                .width(rail_item)
                .height(navigation.item_height)
                .style(move |_, status| {
                    let active = screen == self.screen;
                    widget::button::Style {
                        background: Some(
                            if active {
                                palette.accent_dim
                            } else if matches!(status, widget::button::Status::Hovered) {
                                palette.hover
                            } else {
                                palette.side
                            }
                            .into(),
                        ),
                        text_color: if active {
                            palette.accent_text
                        } else {
                            palette.text2
                        },
                        border: iced::Border {
                            width: if self.rail_focus == Some(index) {
                                2.0
                            } else {
                                0.0
                            },
                            color: palette.accent_text,
                            radius: design::RADIUS.into(),
                        },
                        ..Default::default()
                    }
                })
                .into(),
            );
        }
        let rec = Message::RecMenu;
        items.push(
            tooltip(
                button(
                    column![
                        text(if self.status.state == "paused" {
                            "Ⅱ"
                        } else if matches!(
                            self.status.state.as_str(),
                            "tracking" | "away" | "locked" | "asleep"
                        ) {
                            "●"
                        } else {
                            "○"
                        })
                        .size(design::BODY),
                        text("REC").size(design::CAPTION).font(design::MONO)
                    ]
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
                )
                .on_press(rec)
                .width(rail_item)
                .height(navigation.item_height)
                .style(move |_, _| widget::button::Style {
                    background:
                        Some(
                            if self.status.state == "paused"
                                || self.motion.active(motion::Key::Pause)
                            {
                                motion::mix(
                                    palette.accent_dim,
                                    palette.away_dim,
                                    self.motion.value(
                                        motion::Key::Pause,
                                        f32::from(self.status.state == "paused"),
                                    ),
                                )
                            } else if matches!(
                                self.status.state.as_str(),
                                "tracking" | "away" | "locked" | "asleep"
                            ) {
                                palette.accent_dim
                            } else {
                                Color {
                                    a: 0.1,
                                    ..palette.danger
                                }
                            }
                            .into(),
                        ),
                    text_color: if self.status.state == "paused"
                        || self.motion.active(motion::Key::Pause)
                    {
                        motion::mix(
                            palette.accent_text,
                            palette.away_text,
                            self.motion.value(
                                motion::Key::Pause,
                                f32::from(self.status.state == "paused"),
                            ),
                        )
                    } else if matches!(
                        self.status.state.as_str(),
                        "tracking" | "away" | "locked" | "asleep"
                    ) {
                        palette.accent_text
                    } else {
                        palette.danger_text
                    },
                    border: iced::Border {
                        width: if self.rail_focus == Some(5) { 2.0 } else { 0.0 },
                        color: palette.accent_text,
                        radius: design::RADIUS.into(),
                    },
                    ..Default::default()
                }),
                text(self.status_label()),
                tooltip::Position::Right,
            )
            .into(),
        );
        let sidebar: Element<'_, Message> = if narrow {
            container(scrollable(row(items).spacing(design::SMALL_GAP)).direction(
                scrollable::Direction::Horizontal(scrollable::Scrollbar::default()),
            ))
            .padding(design::TOP_PADDING)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(palette.side.into()),
                ..Default::default()
            })
            .into()
        } else {
            container(column(items).spacing(design::SMALL_GAP))
                .padding(design::RAIL_INSET)
                .width(rail_width)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(palette.side.into()),
                    ..Default::default()
                })
                .into()
        };
        let content = if self.screen == "Day" {
            self.day_view(narrow)
        } else if self.screen == "Week" {
            self.week_view(narrow)
        } else if matches!(self.screen, "Overview" | "Reports") {
            self.reports_view()
        } else if matches!(self.screen, "Tags" | "Rules") {
            self.management_view(narrow)
        } else if self.screen == "Settings" {
            self.settings(narrow)
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
        // shortcut: legacy bodies retain desktop width; replace overflow per screen in slices 2–5.
        let content: Element<'_, Message> =
            if narrow && !matches!(self.screen, "Day" | "Week" | "Tags" | "Rules" | "Settings") {
                scrollable(
                    container(content)
                        .width(design::LEGACY_WIDTH)
                        .height(design::NARROW_BODY_HEIGHT),
                )
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .into()
            } else {
                content
            };
        let mut main = column![].spacing(design::GAP);
        if !matches!(self.screen, "Day" | "Week" | "Tags" | "Rules" | "Settings") {
            main = main.push(text(tr(self.screen)).size(design::TITLE));
            if self.search_open {
                main = main.push(
                    text_input(&tr("Search activity · Ctrl/Cmd F"), &self.search)
                        .on_input(Message::Search)
                        .id("activity-search"),
                );
            }
        }
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
                        .spacing(design::CONTROL_GAP)
                    ]
                    .spacing(design::CONTROL_GAP),
                )
                .padding(design::RAIL_PADDING)
                .style(panel),
            );
        }
        if self.away_range.is_some() {
            let mut recent = row![].spacing(design::CONTROL_GAP);
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
                    .size(design::BODY)
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
                button(text(localization::error(error)).size(design::META))
                    .on_press(Message::Screen("Settings"))
                    .style(design::destructive),
            );
        }
        main = main.push(content).height(
            if narrow && matches!(self.screen, "Week" | "Tags" | "Rules" | "Settings") {
                Length::Shrink
            } else {
                Length::Fill
            },
        );
        // Track the background as a quad so tiny-skia repaints reused buffers after theme changes.
        let title = if self.screen == "Week" {
            "Week"
        } else if self.screen == "Day" {
            "Whole day"
        } else {
            self.screen
        };
        let details = column![
            text(tr(title)).size(design::HEADING),
            space::vertical(),
            text(tr("Activity stays on this computer."))
                .size(design::LABEL)
                .color(palette.text3)
        ];
        let right_content: Element<'_, Message> = if self.screen == "Day" {
            self.day_summary()
        } else if self.screen == "Week" {
            self.week_summary()
        } else if matches!(self.screen, "Tags" | "Rules") {
            self.management_panel()
        } else if self.screen == "Settings" {
            self.settings_navigation(narrow)
        } else {
            details.into()
        };
        let right = container(right_content)
            .padding(design::CARD_PADDING)
            .width(if narrow {
                Length::Fill
            } else {
                Length::Fixed(if self.screen == "Settings" {
                    design::SETTINGS_PANEL
                } else {
                    design::PANEL
                })
            })
            .height(
                if narrow && matches!(self.screen, "Day" | "Week" | "Tags" | "Rules") {
                    Length::Fixed(design::NARROW_BODY_HEIGHT)
                } else if narrow {
                    Length::Shrink
                } else {
                    Length::Fill
                },
            )
            .style(move |_| container::Style {
                background: Some(palette.side.into()),
                ..Default::default()
            });
        let right: Element<'_, Message> = if narrow {
            right.into()
        } else {
            widget::stack![
                right,
                container(space::vertical())
                    .width(1)
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(palette.line.into()),
                        ..Default::default()
                    })
            ]
            .into()
        };
        let main = container(main)
            .padding(design::PADDING)
            .width(Length::Fill)
            .height(Length::Fill);
        let base: Element<'_, Message> = if narrow {
            let main = if matches!(self.screen, "Week" | "Tags" | "Rules" | "Settings") {
                main.height(Length::Shrink)
            } else {
                main.height(design::NARROW_MAIN_HEIGHT)
            };
            container(
                scrollable(if self.screen == "Settings" {
                    column![sidebar, right, main].spacing(design::GAP)
                } else {
                    column![sidebar, main, right].spacing(design::GAP)
                })
                .style(design::scrollbars),
            )
        } else {
            container(if self.screen == "Settings" {
                row![sidebar, right, main]
            } else {
                row![sidebar, main, right]
            })
        }
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(palette.bg.into()),
            text_color: Some(palette.text),
            ..Default::default()
        })
        .into();
        let base = if self.rec_menu || self.motion.active(motion::Key::Menu) {
            widget::stack![
                base,
                container(motion::effect(
                    self.rec_popover(),
                    motion::Visual {
                        opacity: self
                            .motion
                            .value(motion::Key::Menu, f32::from(self.rec_menu)),
                        offset: design::MOTION_OFFSET
                            * (1.0
                                - self
                                    .motion
                                    .value(motion::Key::Menu, f32::from(self.rec_menu))),
                        ..Default::default()
                    },
                    palette.bg
                ))
                .padding(iced::Padding {
                    left: if narrow {
                        design::PADDING as f32
                    } else {
                        rail_width + design::CONTROL_GAP as f32
                    },
                    top: if narrow {
                        design::RAIL_ITEM.max(navigation.item_height)
                            + 2.0 * design::TOP_PADDING as f32
                            + design::CONTROL_GAP as f32
                    } else {
                        design::RAIL_INSET as f32
                            + design::RAIL_ITEM
                            + 5.0 * navigation.item_height
                            + 6.0 * design::SMALL_GAP as f32
                    },
                    ..iced::Padding::ZERO
                })
            ]
            .into()
        } else {
            base
        };
        if self.editor.is_some() {
            let dialog = container(scrollable(column![
                self.tag_editor(),
                text(localization::error(&self.error)).color(Color::from_rgb8(200, 75, 75))
            ]))
            .width(650)
            .max_height(520)
            .padding(design::RAIL_PADDING)
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
    let p = design::Palette::new(dark);
    Theme::custom(
        "Draugr Time",
        iced::theme::Palette {
            background: p.bg,
            text: p.text,
            primary: p.accent,
            success: p.accent,
            danger: p.danger,
            warning: p.away,
        },
    )
}

#[cfg(test)]
mod theme_render_tests {
    use super::*;
    use iced::advanced::{layout, mouse, renderer, widget::Tree, Layout, Renderer as _};

    #[test]
    fn rail_keyboard_focus_moves_and_activates_without_changing_day_state() {
        let (mut state, _) = State::boot();
        let day = state.day;
        let key = |named| Message::Key(keyboard::Key::Named(named), keyboard::Modifiers::empty());
        let _ = state.update(key(keyboard::key::Named::F6));
        assert_eq!(state.rail_focus, Some(0));
        let _ = state.update(key(keyboard::key::Named::ArrowDown));
        assert_eq!(state.rail_focus, Some(1));
        assert_eq!(state.screen, "Day");
        let _ = state.update(key(keyboard::key::Named::Enter));
        assert_eq!(state.screen, "Week");
        assert_eq!(state.day, day);
        let _ = state.update(key(keyboard::key::Named::Escape));
        assert_eq!(state.rail_focus, None);
    }

    #[test]
    fn day_range_controls_and_lane_expansion_preserve_selection() {
        let (mut state, _) = State::boot();
        state.all = model::fixture(state.day);
        state.rebuild();
        let selection = Range::new(state.day.start + 1000, state.day.start + 5000);
        state.selection = Some(selection);
        let _ = state.update(Message::DayRange(2));
        assert_eq!(state.view, selection);
        let _ = state.update(Message::ExpandTrack(2));
        assert!(state.expanded_tracks[2]);
        assert_eq!(state.selection, Some(selection));
        let _ = state.update(Message::DayRange(0));
        assert_eq!(state.view, state.day);
    }

    #[test]
    fn shell_keeps_the_rail_and_panel_at_their_spec_widths() {
        let (mut state, _) = State::boot();
        state.fixture = true;
        state.theme = native_theme(true);
        let size = iced::Size::new(1280.0, 820.0);
        let viewport = iced::Rectangle::with_size(size);
        let mut renderer = iced::Renderer::new(design::UI, 16.0.into());
        let theme = native_theme(true);
        let mut view = state.view_sized(size.width);
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
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
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        let side = iced::Background::Color(design::Palette::new(true).side);
        let rectangles: Vec<_> = renderer
            .layers()
            .iter()
            .flat_map(|layer| &layer.quads)
            .filter(|(_, background)| *background == side)
            .map(|(quad, _)| quad.bounds)
            .collect();
        assert!(rectangles
            .iter()
            .any(|r| r.x == 0.0 && r.width >= design::RAIL && r.height == 820.0));
        assert!(rectangles
            .iter()
            .any(|r| r.x == 936.0 && r.width == 344.0 && r.height == 820.0));
    }

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
    let p = design::Palette::new(theme.palette().background.r < 0.5);
    container::Style {
        background: Some(p.surf.into()),
        border: iced::Border {
            width: 1.0,
            color: p.line,
            radius: design::RADIUS.into(),
        },
        ..Default::default()
    }
}

pub fn run() -> iced::Result {
    memory::limit_arenas();
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
        .font(include_bytes!("../../fonts/InterTight.ttf").as_slice())
        .font(include_bytes!("../../fonts/JetBrainsMono-Regular.ttf").as_slice())
        .default_font(design::UI)
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
