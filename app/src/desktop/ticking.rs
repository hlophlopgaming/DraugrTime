//! When the desktop GUI needs its one-second timer.
//!
//! Linux and Windows sample activity on a background worker that pushes status
//! changes to the GUI (`tray::Event::Status`), so an open window only needs the
//! timer to load history: once for a newly shown day, and repeatedly while the
//! focused Day screen shows a day that is still being recorded. Everything else wakes
//! the GUI through messages. macOS samples on the main thread and always ticks;
//! diagnostic launch modes keep ticking because they drive their checks from it.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TickDemand {
    /// Lifecycle/benchmark diagnostics, or main-thread sampling (macOS).
    pub always: bool,
    pub window_open: bool,
    /// Smoke/runtime/onboarding checks that run while a window is open.
    pub window_diagnostics: bool,
    /// History can be loaded: a focused window, a runtime and no fixture.
    pub can_load: bool,
    /// The shown day's history has been loaded at least once.
    pub day_loaded: bool,
    /// The shown day contains the current time, so its history still grows.
    pub live_day: bool,
    pub on_day_screen: bool,
}

impl TickDemand {
    pub(super) fn needed(self) -> bool {
        if self.always {
            return true;
        }
        if !self.window_open {
            return false;
        }
        if self.window_diagnostics {
            return true;
        }
        if !self.can_load {
            return false;
        }
        !self.day_loaded || (self.live_day && self.on_day_screen)
    }
}

#[cfg(test)]
mod tests {
    use super::TickDemand;

    fn live_day_window() -> TickDemand {
        TickDemand {
            window_open: true,
            can_load: true,
            day_loaded: true,
            live_day: true,
            on_day_screen: true,
            ..TickDemand::default()
        }
    }

    #[test]
    fn hidden_window_does_not_tick() {
        let demand = TickDemand {
            window_open: false,
            ..live_day_window()
        };
        assert!(!demand.needed());
    }

    #[test]
    fn day_screen_showing_a_recorded_day_keeps_ticking() {
        assert!(live_day_window().needed());
    }

    #[test]
    fn loaded_past_day_stops_ticking() {
        let demand = TickDemand {
            live_day: false,
            ..live_day_window()
        };
        assert!(!demand.needed());
    }

    #[test]
    fn unloaded_day_ticks_until_its_first_load_on_any_screen() {
        let demand = TickDemand {
            day_loaded: false,
            live_day: false,
            on_day_screen: false,
            ..live_day_window()
        };
        assert!(demand.needed());
    }

    #[test]
    fn other_screens_do_not_reload_a_live_day() {
        let demand = TickDemand {
            on_day_screen: false,
            ..live_day_window()
        };
        assert!(!demand.needed());
    }

    #[test]
    fn preview_without_runtime_never_ticks_outside_diagnostics() {
        let demand = TickDemand {
            can_load: false,
            day_loaded: false,
            ..live_day_window()
        };
        assert!(!demand.needed());
    }

    #[test]
    fn window_diagnostics_tick_only_with_a_window() {
        let open = TickDemand {
            window_open: true,
            window_diagnostics: true,
            ..TickDemand::default()
        };
        assert!(open.needed());
        assert!(!TickDemand {
            window_open: false,
            ..open
        }
        .needed());
    }

    #[test]
    fn lifecycle_benchmark_and_macos_always_tick() {
        assert!(TickDemand {
            always: true,
            ..TickDemand::default()
        }
        .needed());
    }
}
