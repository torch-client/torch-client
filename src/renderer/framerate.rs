use std::time::Duration;

use bevy::ecs::system::NonSendMarker;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseMotion, MouseWheel};
use bevy::input::touch::TouchInput;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;

use crate::gui::InactivityFpsLimit;
use crate::platform::time::Instant;

const OUT_OF_LEVEL_MENU_LIMIT: u32 = 60;
const ICONIFIED_WINDOW_LIMIT: u32 = 10;
const AFK_LIMIT: u32 = 30;
const LONG_AFK_LIMIT: u32 = 10;
const AFK_THRESHOLD: Duration = Duration::from_secs(60);
const LONG_AFK_THRESHOLD: Duration = Duration::from_secs(600);
const ICONIFY_POLL: Duration = Duration::from_secs(1);

fn limit_for(
    framerate_limit: u32,
    inactivity: InactivityFpsLimit,
    iconified: bool,
    idle: Duration,
    out_of_level_menu: bool,
) -> u32 {
    if iconified {
        return ICONIFIED_WINDOW_LIMIT;
    }
    if inactivity == InactivityFpsLimit::Afk {
        if idle > LONG_AFK_THRESHOLD {
            return LONG_AFK_LIMIT;
        }
        if idle > AFK_THRESHOLD {
            return framerate_limit.min(AFK_LIMIT);
        }
    }
    if out_of_level_menu {
        return if cfg!(feature = "mobile_ui") {
            framerate_limit.min(OUT_OF_LEVEL_MENU_LIMIT)
        } else {
            OUT_OF_LEVEL_MENU_LIMIT
        };
    }
    framerate_limit
}

#[derive(Resource)]
pub struct FramerateLimitTracker {
    latest_input: Instant,
    iconified: bool,
    latest_poll: Instant,
}

impl Default for FramerateLimitTracker {
    fn default() -> Self {
        let now = Instant::now();
        FramerateLimitTracker {
            latest_input: now,
            iconified: false,
            latest_poll: now,
        }
    }
}

impl FramerateLimitTracker {
    pub fn limit(&self, gui: &crate::gui::GuiState, in_world: bool) -> u32 {
        limit_for(
            gui.options.max_fps,
            gui.options.inactivity_fps_limit,
            self.iconified,
            Instant::now().duration_since(self.latest_input),
            !in_world && !matches!(gui.screen, crate::gui::Screen::None),
        )
    }
}

pub fn track_input(
    mut tracker: ResMut<FramerateLimitTracker>,
    mut keys: MessageReader<KeyboardInput>,
    mut buttons: MessageReader<MouseButtonInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    mut touches: MessageReader<TouchInput>,
    window: Query<(Entity, &Window), With<PrimaryWindow>>,
    _main_thread: NonSendMarker,
) {
    let primary = window.single().ok();
    let focused = primary.is_some_and(|(_, window)| window.focused);
    let moved = motion.read().count() != 0 && focused;
    let pressed = keys.read().count()
        + buttons.read().count()
        + wheel.read().count()
        + touches.read().count();
    let input = moved || pressed != 0;
    let now = Instant::now();
    if input {
        tracker.latest_input = now;
    }
    if (input && tracker.iconified) || now.duration_since(tracker.latest_poll) >= ICONIFY_POLL {
        tracker.latest_poll = now;
        tracker.iconified = primary.is_some_and(|(entity, _)| {
            WINIT_WINDOWS
                .with_borrow(|windows| windows.get_window(entity)?.is_minimized())
                .unwrap_or(false)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: u32 = 144;
    const NOW: Duration = Duration::ZERO;
    const AFK: InactivityFpsLimit = InactivityFpsLimit::Afk;
    const MINIMIZED: InactivityFpsLimit = InactivityFpsLimit::Minimized;

    #[test]
    fn nothing_going_on_keeps_the_option() {
        assert_eq!(limit_for(CAP, AFK, false, NOW, false), CAP);
    }

    #[test]
    fn iconified_is_ten_whatever_the_option_says() {
        assert_eq!(limit_for(CAP, AFK, true, NOW, false), 10);
        assert_eq!(limit_for(CAP, MINIMIZED, true, NOW, true), 10);
    }

    #[test]
    fn afk_steps_past_each_threshold_and_not_at_it() {
        assert_eq!(limit_for(CAP, AFK, false, AFK_THRESHOLD, false), CAP);
        assert_eq!(
            limit_for(
                CAP,
                AFK,
                false,
                AFK_THRESHOLD + Duration::from_millis(1),
                false
            ),
            30
        );
        assert_eq!(limit_for(CAP, AFK, false, LONG_AFK_THRESHOLD, false), 30);
        assert_eq!(
            limit_for(
                CAP,
                AFK,
                false,
                LONG_AFK_THRESHOLD + Duration::from_millis(1),
                false
            ),
            10
        );
    }

    #[test]
    fn short_afk_never_raises_a_lower_cap() {
        assert_eq!(
            limit_for(20, AFK, false, Duration::from_secs(61), false),
            20
        );
    }

    #[test]
    fn minimized_mode_ignores_idling() {
        assert_eq!(
            limit_for(CAP, MINIMIZED, false, Duration::from_secs(3600), false),
            CAP
        );
    }

    #[test]
    fn afk_outranks_the_menu_limit() {
        assert_eq!(
            limit_for(CAP, AFK, false, Duration::from_secs(61), true),
            30
        );
    }

    #[cfg(not(feature = "mobile_ui"))]
    #[test]
    fn a_menu_outside_a_level_is_a_flat_sixty() {
        assert_eq!(limit_for(CAP, AFK, false, NOW, true), 60);
        assert_eq!(limit_for(30, AFK, false, NOW, true), 60);
    }
}
