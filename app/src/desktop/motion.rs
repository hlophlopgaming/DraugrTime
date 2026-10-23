use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Key {
    Track(usize),
    Summary,
    Tree(i64),
    Tooltip,
    Selection,
    Tag,
    Tab,
    Range,
    Date,
    Menu,
    Pause,
    Dialog,
    Highlight,
}
#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    delay: Duration,
    duration: Duration,
    linear: bool,
    symmetric: bool,
}
impl Tween {
    fn value(self, now: Instant) -> f32 {
        let elapsed = now
            .saturating_duration_since(self.start)
            .saturating_sub(self.delay);
        let t = (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        let t = if self.linear {
            t
        } else if self.symmetric {
            t * t * (3.0 - 2.0 * t)
        } else {
            ease_out(t)
        };
        self.from + (self.to - self.from) * t
    }
    fn complete(self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= self.delay + self.duration
    }
}
// Solve the spec's cubic-bezier(0.2, 0.7, 0.2, 1) in x before evaluating y.
fn ease_out(x: f32) -> f32 {
    let cubic = |t: f32, a: f32, b: f32| {
        3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..16 {
        let t = (low + high) * 0.5;
        if cubic(t, 0.2, 0.2) < x {
            low = t;
        } else {
            high = t;
        }
    }
    if x == 0.0 || x == 1.0 {
        x
    } else {
        cubic((low + high) * 0.5, 0.7, 1.0)
    }
}
pub(super) struct Ui {
    pub reduced: bool,
    now: Instant,
    active: BTreeMap<Key, Tween>,
}
impl Default for Ui {
    fn default() -> Self {
        Self {
            reduced: true,
            now: Instant::now(),
            active: BTreeMap::new(),
        }
    }
}
impl Ui {
    pub fn value(&self, key: Key, fallback: f32) -> f32 {
        self.active
            .get(&key)
            .map(|t| t.value(self.now))
            .unwrap_or(fallback)
    }
    pub fn active(&self, key: Key) -> bool {
        self.active.contains_key(&key)
    }
    pub fn running(&self, visible: bool) -> bool {
        visible
            && !self.reduced
            && self
                .active
                .values()
                .any(|t| self.now.saturating_duration_since(t.start) >= t.delay)
    }
    pub fn cancel(&mut self, key: Key) {
        self.active.remove(&key);
    }
    pub fn clear(&mut self) {
        self.active.clear();
    }
    pub fn set_reduced(&mut self, reduced: bool) {
        self.reduced = reduced;
        if reduced {
            self.clear();
        }
    }
    pub fn advance(&mut self, now: Instant) {
        self.now = now;
        self.active.retain(|_, t| !t.complete(now));
    }
    pub fn start(&mut self, key: Key, from: f32, to: f32, visible: bool) {
        self.start_at(key, from, to, Instant::now(), visible);
    }
    fn start_at(&mut self, key: Key, from: f32, to: f32, now: Instant, visible: bool) {
        self.now = now;
        if self.reduced || !visible {
            self.active.remove(&key);
            return;
        }
        let from = self.value(key, from);
        if (from - to).abs() < f32::EPSILON {
            self.active.remove(&key);
            return;
        }
        let ms = match key {
            Key::Track(_) => {
                if to > from {
                    260
                } else {
                    200
                }
            }
            Key::Summary | Key::Tree(_) | Key::Dialog => 180,
            Key::Tooltip => 100,
            Key::Selection => 120,
            Key::Tag | Key::Pause => 200,
            Key::Tab | Key::Range => 160,
            Key::Date => 140,
            Key::Highlight => 150,
            Key::Menu => {
                if to > from {
                    140
                } else {
                    90
                }
            }
        };
        let delay = if key == Key::Tooltip {
            Duration::from_millis(super::design::TOOLTIP_DELAY)
        } else {
            Duration::ZERO
        };
        self.active.insert(
            key,
            Tween {
                from,
                to,
                start: now,
                delay,
                duration: Duration::from_millis(ms),
                linear: matches!(key, Key::Date | Key::Highlight),
                symmetric: matches!(key, Key::Tab | Key::Range | Key::Pause),
            },
        );
    }
}

pub(super) trait MotionPreference {
    fn reduced(&self) -> bool;
    fn changes(&self) -> iced::Subscription<bool> {
        iced::Subscription::none()
    }
}
pub(super) struct SystemMotionPreference;
impl MotionPreference for SystemMotionPreference {
    fn changes(&self) -> iced::Subscription<bool> {
        #[cfg(target_os = "linux")]
        {
            iced::Subscription::run(portal_motion_changes)
        }
        #[cfg(not(target_os = "linux"))]
        {
            iced::Subscription::none()
        }
    }
    fn reduced(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            let Ok(connection) = zbus::blocking::Connection::session() else {
                return true;
            };
            let Ok(proxy) = zbus::blocking::Proxy::new(
                &connection,
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings",
            ) else {
                return true;
            };
            for (namespace, key) in [
                ("org.gnome.desktop.interface", "enable-animations"),
                ("org.kde.kdeglobals.KDE", "AnimationDurationFactor"),
            ] {
                let result: zbus::Result<zbus::zvariant::OwnedValue> =
                    proxy.call("ReadOne", &(namespace, key));
                if let Ok(value) = result {
                    if let Some(reduced) = setting_reduced(namespace, key, value) {
                        return reduced;
                    }
                }
            }
            true
        }
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
            };
            let mut enabled: windows_sys::core::BOOL = 0;
            // SAFETY: SPI writes one BOOL into this valid local pointer.
            unsafe {
                SystemParametersInfoW(
                    SPI_GETCLIENTAREAANIMATION,
                    0,
                    (&mut enabled as *mut windows_sys::core::BOOL).cast(),
                    0,
                ) == 0
                    || enabled == 0
            }
        }
        #[cfg(target_os = "macos")]
        {
            objc2_app_kit::NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            true
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Visual {
    pub opacity: f32,
    pub height: f32,
    pub offset: f32,
    pub scale: f32,
    pub segment: Option<f32>,
}
impl Default for Visual {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            height: 1.0,
            offset: 0.0,
            scale: 1.0,
            segment: None,
        }
    }
}
pub(super) fn effect<'a>(
    content: impl Into<iced::Element<'a, super::Message>>,
    visual: Visual,
    background: iced::Color,
) -> iced::Element<'a, super::Message> {
    iced::Element::new(Effect {
        content: content.into(),
        visual,
        background,
    })
}
struct Effect<'a> {
    content: iced::Element<'a, super::Message>,
    visual: Visual,
    background: iced::Color,
}
use iced::advanced::{
    layout, mouse, renderer,
    widget::{tree, Operation, Tree},
    Clipboard, Layout, Shell, Widget,
};
use iced::{Event, Length, Rectangle, Renderer, Size, Theme};
impl Widget<super::Message, Theme, Renderer> for Effect<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[self.content.as_widget()]);
    }
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let size = child.size();
        layout::Node::with_children(
            Size::new(size.width, size.height * self.visual.height),
            vec![child],
        )
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if self.visual.opacity > 0.0 && self.visual.height >= 1.0 {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        }
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, super::Message>,
        viewport: &Rectangle,
    ) {
        if self.visual.opacity <= 0.0 || self.visual.height < 1.0 {
            return;
        }
        let Some(viewport) = viewport.intersection(&layout.bounds()) else {
            return;
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            &viewport,
        );
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let Some(bounds) = viewport.intersection(&layout.bounds()) else {
            return;
        };
        if self.visual.opacity <= 0.0 {
            return;
        }
        renderer.with_layer(bounds, |renderer| {
            if let Some(position) = self.visual.segment {
                let children: Vec<_> = layout
                    .children()
                    .next()
                    .unwrap()
                    .children()
                    .map(|child| child.bounds())
                    .collect();
                if !children.is_empty() {
                    let position = position.clamp(0.0, (children.len() - 1) as f32);
                    let a = children[position.floor() as usize];
                    let b = children[position.ceil() as usize];
                    let t = position.fract();
                    let thumb = Rectangle {
                        x: a.x + (b.x - a.x) * t,
                        y: a.y,
                        width: a.width + (b.width - a.width) * t,
                        height: a.height,
                    };
                    let palette = super::design::Palette::new(theme.palette().background.r < 0.5);
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: thumb,
                            border: iced::Border::default().rounded(super::design::SEGMENT_RADIUS),
                            ..Default::default()
                        },
                        palette.hover,
                    );
                }
            }
            let center = layout.bounds().center();
            let transform =
                iced::Transformation::translate(center.x, center.y + self.visual.offset)
                    * iced::Transformation::scale(self.visual.scale)
                    * iced::Transformation::translate(-center.x, -center.y);
            renderer.with_transformation(transform, |renderer| {
                self.content.as_widget().draw(
                    &tree.children[0],
                    renderer,
                    theme,
                    style,
                    layout.children().next().unwrap(),
                    cursor,
                    &bounds,
                );
            });
            if self.visual.opacity < 1.0 {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..Default::default()
                    },
                    self.background.scale_alpha(1.0 - self.visual.opacity),
                );
            }
        });
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.visual.opacity <= 0.0 || self.visual.height < 1.0 {
            return mouse::Interaction::default();
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, super::Message, Theme, Renderer>> {
        if self.visual.opacity <= 0.0 || self.visual.height < 1.0 {
            return None;
        }
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

pub(super) fn read_preference() -> iced::Task<super::Message> {
    iced::Task::perform(
        async {
            tokio::task::spawn_blocking(|| SystemMotionPreference.reduced())
                .await
                .unwrap_or(true)
        },
        super::Message::MotionChanged,
    )
}
impl super::State {
    pub(super) fn animate(&mut self, key: Key, from: f32, to: f32) {
        self.motion.start(
            key,
            from,
            to,
            self.window.is_some()
                && self.motion_visible
                && !self.loading
                && !self.management.loading
                && !self.report_ui.loading,
        );
    }
    pub(super) fn tab_position(&self) -> f32 {
        match self.tab {
            1 => 0.0,
            2 => 1.0,
            _ => 2.0,
        }
    }
}
#[cfg(target_os = "linux")]
fn portal_motion_changes() -> impl iced::futures::Stream<Item = bool> {
    use iced::futures::{SinkExt, StreamExt};
    iced::stream::channel(4, async move |mut output| {
        let Ok(connection) = zbus::Connection::session().await else {
            return;
        };
        let Ok(proxy) = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .await
        else {
            return;
        };
        let Ok(mut changes) = proxy.receive_signal("SettingChanged").await else {
            return;
        };
        while let Some(signal) = changes.next().await {
            let Ok((namespace, key, value)) =
                signal
                    .body()
                    .deserialize::<(String, String, zbus::zvariant::OwnedValue)>()
            else {
                continue;
            };
            let reduced = setting_reduced(&namespace, &key, value);
            if let Some(reduced) = reduced {
                if output.send(reduced).await.is_err() {
                    return;
                }
            }
        }
    })
}

pub(super) fn track_height(track: usize, lanes: usize, progress: f32) -> f32 {
    let collapsed = super::design::track_height(track, 0);
    if lanes == 0 {
        collapsed
    } else {
        collapsed + (super::design::track_height(track, lanes) - collapsed) * progress
    }
}
pub(super) fn chevron(
    progress: f32,
    color: iced::Color,
) -> iced::widget::Container<'static, super::Message> {
    iced::widget::container(iced::widget::svg(iced::widget::svg::Handle::from_memory(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 12"><path d="m4 2 4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>"#.to_vec()))
        .width(super::design::LABEL).height(super::design::LABEL)
        .rotation(iced::Radians(progress * std::f32::consts::FRAC_PI_2))
        .style(move |_, _| iced::widget::svg::Style { color: Some(color) }))
        .center_x(Length::Shrink).center_y(Length::Shrink)
}

pub(super) fn mix(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
    iced::Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

#[cfg(target_os = "linux")]
fn setting_reduced(namespace: &str, key: &str, value: zbus::zvariant::OwnedValue) -> Option<bool> {
    match (namespace, key) {
        ("org.gnome.desktop.interface", "enable-animations") => {
            bool::try_from(value).ok().map(|value| !value)
        }
        ("org.kde.kdeglobals.KDE", "AnimationDurationFactor") => f64::try_from(value)
            .ok()
            .map(|value| !value.is_finite() || value <= 0.0),
        _ => None,
    }
}

impl super::State {
    pub(super) fn finish_motion(&mut self) {
        if self.motion.active(Key::Track(2)) || self.motion.active(Key::Track(3)) {
            self.cache.clear();
            self.grid_cache.clear();
        }
        self.motion.clear();
        if self.summary_closing.take().is_some() {
            self.summary();
        }
    }
}

// Retain the current mixture on rapid retargets, rather than jumping to the old target.
pub(super) fn highlight_snapshot(
    from: &mut Vec<(Option<String>, f32)>,
    target: Option<&str>,
    progress: f32,
) {
    for (_, weight) in from.iter_mut() {
        *weight *= 1.0 - progress;
    }
    if let Some((_, weight)) = from.iter_mut().find(|(key, _)| key.as_deref() == target) {
        *weight += progress;
    } else {
        from.push((target.map(str::to_owned), progress));
    }
    from.retain(|(_, weight)| *weight > f32::EPSILON);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_stop_and_do_not_tick_when_hidden_or_reduced() {
        let start = Instant::now();
        let mut ui = Ui {
            reduced: false,
            ..Default::default()
        };
        ui.start_at(Key::Track(2), 0.0, 1.0, start, true);
        assert!(ui.running(true));
        assert!(!ui.running(false));
        ui.advance(start + Duration::from_millis(130));
        assert!(ui.value(Key::Track(2), 1.0) > 0.5);
        ui.advance(start + Duration::from_millis(260));
        assert!(!ui.running(true));
        assert_eq!(ui.value(Key::Track(2), 1.0), 1.0);
        ui.set_reduced(true);
        ui.start_at(Key::Menu, 0.0, 1.0, start, true);
        assert!(!ui.running(true));
        assert_eq!(ui.value(Key::Menu, 1.0), 1.0);
    }
    #[test]
    fn reversing_keeps_the_current_value_and_uses_collapse_duration() {
        let start = Instant::now();
        let mut ui = Ui {
            reduced: false,
            ..Default::default()
        };
        ui.start_at(Key::Track(3), 0.0, 1.0, start, true);
        let turn = start + Duration::from_millis(80);
        ui.advance(turn);
        let before = ui.value(Key::Track(3), 1.0);
        ui.start_at(Key::Track(3), 1.0, 0.0, turn, true);
        assert!((ui.value(Key::Track(3), 0.0) - before).abs() < 0.001);
        ui.advance(turn + Duration::from_millis(200));
        assert_eq!(ui.value(Key::Track(3), 0.0), 0.0);
        assert!(!ui.running(true));
    }
    #[test]
    fn tooltip_waits_then_fades_and_unknown_platform_settings_skip_motion() {
        let start = Instant::now();
        let mut ui = Ui::default();
        assert!(ui.reduced);
        ui.reduced = false;
        ui.start_at(Key::Tooltip, 0.0, 1.0, start, true);
        assert!(
            !ui.running(true),
            "hover delay uses one wakeup, not frame ticks"
        );
        ui.advance(start + Duration::from_millis(119));
        assert_eq!(ui.value(Key::Tooltip, 1.0), 0.0);
        ui.advance(start + Duration::from_millis(170));
        assert!(ui.value(Key::Tooltip, 1.0) > 0.0);
        ui.advance(start + Duration::from_millis(220));
        assert!(!ui.running(true));
        ui.start_at(Key::Selection, 0.0, 1.0, start, false);
        assert!(!ui.running(true));
    }
    fn fixture_state() -> super::super::State {
        use super::super::*;
        let (mut state, _) = State::boot();
        state.fixture = true;
        state.window = Some(window::Id::unique());
        state.motion_visible = true;
        state.motion.set_reduced(false);
        state.all = model::fixture(state.day);
        state.rebuild();
        state
    }

    #[test]
    fn track_reversal_reduced_motion_and_focus_loss_preserve_data_and_selection() {
        use super::super::*;
        let mut state = fixture_state();
        state.selection = Some(Range::new(state.day.start, state.day.start + 60_000));
        let before = state.selection;
        let count = state.all[2].segments.len();
        let _ = state.update(Message::ExpandTrack(2));
        assert!(state.motion.running(true));
        let _ = state.update(Message::MotionFrame(
            Instant::now() + Duration::from_millis(130),
        ));
        let value = state.motion.value(Key::Track(2), 1.0);
        assert!(value > 0.0 && value < 1.0);
        let _ = state.update(Message::ExpandTrack(2));
        assert!(
            !state.day_lanes()[2].is_empty(),
            "collapsing lanes stay until the final frame"
        );
        let _ = state.update(Message::MotionChanged(true));
        assert!(!state.motion.running(true));
        assert!(state.day_lanes()[2].is_empty());
        assert_eq!(state.selection, before);
        assert_eq!(state.all[2].segments.len(), count);
        let _ = state.update(Message::MotionChanged(false));
        let _ = state.update(Message::RecMenu);
        assert!(state.motion.running(true));
        let _ = state.update(Message::MotionUnfocused);
        assert!(!state.motion.running(true));
        let _ = state.update(Message::RecMenu);
        assert!(
            !state.motion.running(true),
            "unfocused and minimized windows never start frame ticks"
        );
    }

    #[test]
    fn selection_outline_is_immediate_but_actions_start_only_on_release() {
        use super::super::*;
        let mut state = fixture_state();
        let range = Some(Range::new(state.day.start, state.day.start + 60_000));
        let _ = state.update(Message::SelectDragging(range));
        assert_eq!(state.selection, range);
        assert!(state.selecting);
        assert!(!state.motion.active(Key::Selection));
        let _ = state.update(Message::SelectionReleased);
        assert!(!state.selecting);
        assert!(state.motion.active(Key::Selection));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_motion_values_are_typed_and_unknown_settings_are_ignored() {
        use zbus::zvariant::OwnedValue;
        assert_eq!(
            setting_reduced(
                "org.gnome.desktop.interface",
                "enable-animations",
                OwnedValue::from(false)
            ),
            Some(true)
        );
        assert_eq!(
            setting_reduced(
                "org.gnome.desktop.interface",
                "enable-animations",
                OwnedValue::from(true)
            ),
            Some(false)
        );
        for (factor, reduced) in [(0.0, true), (1.0, false), (f64::NAN, true)] {
            assert_eq!(
                setting_reduced(
                    "org.kde.kdeglobals.KDE",
                    "AnimationDurationFactor",
                    OwnedValue::from(factor)
                ),
                Some(reduced)
            );
        }
        assert_eq!(
            setting_reduced(
                "org.gnome.desktop.interface",
                "enable-animations",
                OwnedValue::from(1_u32)
            ),
            None
        );
        assert_eq!(
            setting_reduced("unrelated", "enable-animations", OwnedValue::from(true)),
            None
        );
    }

    #[test]
    fn effect_clips_layout_and_invisible_controls_do_not_receive_clicks() {
        use super::super::*;
        let renderer = Renderer::new(design::UI, 13.0.into());
        for opacity in [0.0, 1.0] {
            let mut effect = effect(
                button(iced::widget::text("Tag")).on_press(Message::RecMenu),
                Visual {
                    opacity,
                    ..Default::default()
                },
                iced::Color::BLACK,
            );
            let mut tree = Tree::new(effect.as_widget());
            let node = effect.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(200.0, 100.0)),
            );
            let mut messages = Vec::new();
            for button in [
                iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left),
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
            ] {
                effect.as_widget_mut().update(
                    &mut tree,
                    &Event::Mouse(button),
                    Layout::new(&node),
                    iced::mouse::Cursor::Available(iced::Point::new(10.0, 10.0)),
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &Rectangle::with_size(Size::new(200.0, 100.0)),
                );
            }
            assert_eq!(messages.len(), usize::from(opacity == 1.0));
        }
        let mut effect = effect(
            iced::widget::container(iced::widget::space())
                .width(100)
                .height(100),
            Visual {
                height: 0.5,
                ..Default::default()
            },
            iced::Color::BLACK,
        );
        let mut tree = Tree::new(effect.as_widget());
        let node = effect.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0)),
        );
        assert_eq!(node.size(), Size::new(100.0, 50.0));
    }

    fn render(state: &super::super::State, width: u32, height: u32) -> Vec<u8> {
        use iced::advanced::{renderer::Headless as _, Renderer as _};
        let size = Size::new(width as f32, height as f32);
        let viewport = Rectangle::with_size(size);
        let mut renderer = Renderer::new(super::super::design::UI, 13.0.into());
        let mut view = state.view_sized(size.width);
        let mut tree = Tree::new(view.as_widget());
        let node = view.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
        renderer.reset(viewport);
        view.as_widget().draw(
            &tree,
            &mut renderer,
            &state.theme,
            &renderer::Style {
                text_color: state.theme.palette().text,
            },
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &viewport,
        );
        renderer.screenshot(
            iced::Size::new(width, height),
            1.0,
            state.theme.palette().background,
        )
    }

    #[test]
    fn highlight_and_menu_frames_reuse_the_5000_event_geometry() {
        use super::super::*;
        let mut state = fixture_state();
        let _ = render(&state, 1280, 820);
        let builds = state
            .stats
            .base_builds
            .load(std::sync::atomic::Ordering::Relaxed);
        assert!(builds > 0);
        let key = state.all[2].segments[0].key.to_string();
        let _ = state.update(Message::Highlight(key));
        let _ = state.update(Message::MotionFrame(
            Instant::now() + Duration::from_millis(75),
        ));
        let _ = render(&state, 1280, 820);
        assert_eq!(
            state
                .stats
                .base_builds
                .load(std::sync::atomic::Ordering::Relaxed),
            builds
        );
        let _ = state.update(Message::RecMenu);
        let _ = state.update(Message::MotionFrame(
            Instant::now() + Duration::from_millis(70),
        ));
        let _ = render(&state, 1280, 820);
        assert_eq!(
            state
                .stats
                .base_builds
                .load(std::sync::atomic::Ordering::Relaxed),
            builds
        );
    }

    #[test]
    #[ignore = "writes motion review frames; run explicitly with --test-threads=1"]
    fn render_motion_review_screenshots() {
        use super::super::*;
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .unwrap();
        for bytes in [
            include_bytes!("../../fonts/InterTight.ttf").as_slice(),
            include_bytes!("../../fonts/JetBrainsMono-Regular.ttf").as_slice(),
        ] {
            fonts.load_font(std::borrow::Cow::Borrowed(bytes));
        }
        drop(fonts);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots");
        for dark in [true, false] {
            for (width, height) in [(1280, 820), (700, 820), (700, 1600)] {
                let mut state = fixture_state();
                localization::set(if width < 760 {
                    localization::Language::De
                } else {
                    localization::Language::En
                });
                state.theme = native_theme(dark);
                let _ = state.update(Message::ExpandTrack(2));
                let start = state.motion.now;
                for ms in [0, 65, 130, 195, 260] {
                    let _ = state.update(Message::MotionFrame(start + Duration::from_millis(ms)));
                    let rgba = render(&state, width, height);
                    let file = std::fs::File::create(path.join(format!(
                        "m12-motion-{}-{width}x{height}-{ms:03}.png",
                        if dark { "dark" } else { "light" }
                    )))
                    .unwrap();
                    let mut encoder = png::Encoder::new(file, width, height);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(&rgba)
                        .unwrap();
                }
                assert!(
                    state
                        .stats
                        .max_build_us
                        .load(std::sync::atomic::Ordering::Relaxed)
                        < 100_000
                );
                assert!(!state.motion.running(true));
            }
        }
        localization::set(localization::Language::En);
    }
    #[test]
    fn interrupted_highlights_keep_the_current_opacity_mixture() {
        let mut from = Vec::new();
        highlight_snapshot(&mut from, None, 1.0);
        highlight_snapshot(&mut from, Some("editor"), 0.5);
        let opacity = |values: &[(Option<String>, f32)], app: &str| -> f32 {
            values
                .iter()
                .map(|(key, weight)| {
                    weight
                        * if key.as_deref().is_none_or(|key| key == app) {
                            1.0
                        } else {
                            0.16
                        }
                })
                .sum()
        };
        assert!((opacity(&from, "browser") - 0.58).abs() < 0.001);
        highlight_snapshot(&mut from, Some("browser"), 0.25);
        assert!((opacity(&from, "browser") - (0.58 * 0.75 + 0.25)).abs() < 0.001);
        assert!((from.iter().map(|(_, weight)| weight).sum::<f32>() - 1.0).abs() < 0.001);
    }
    #[test]
    fn expansion_starts_without_a_visual_jump() {
        use super::super::*;
        let mut state = fixture_state();
        state.change_date(chrono::NaiveDate::from_ymd_opt(2020, 1, 2).unwrap());
        state.finish_motion();
        let before = render(&state, 1280, 820);
        let _ = state.update(Message::ExpandTrack(2));
        let after = render(&state, 1280, 820);
        assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 0);
    }
}
