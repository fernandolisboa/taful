//! Animations (#59, spec §15.2): a screen fade on navigation, cards dealt
//! onto the felt, chips slid toward the pot and the pot slid to the
//! winner. The `ui.animations` setting picks [`Motion::On`], `Reduced`
//! (fades only) or `Off`; `ui.animation_speed` picks a [`Speed`] that
//! scales every animation and the table replay's pace.
//!
//! GPUI restarts an animation when its element id is new, so each helper
//! takes an id that changes exactly when the thing should animate again
//! (a new card, a new bet, a new screen visit).

use std::cell::Cell;
use std::time::Duration;

use gpui::{Animation, AnimationExt, AnyElement, ElementId, IntoElement, Styled, px};

/// How much the UI animates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    On,
    /// Fades only; nothing moves.
    Reduced,
    Off,
}

impl Motion {
    pub const ALL: [Motion; 3] = [Motion::On, Motion::Reduced, Motion::Off];

    /// The stored setting value (unknown or unset = on).
    pub fn from_setting(s: Option<&str>) -> Motion {
        match s {
            Some("reduced") => Motion::Reduced,
            Some("off") => Motion::Off,
            _ => Motion::On,
        }
    }

    pub fn setting(self) -> &'static str {
        match self {
            Motion::On => "on",
            Motion::Reduced => "reduced",
            Motion::Off => "off",
        }
    }

    /// Fluent id of the option's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Motion::On => "settings-animations-on",
            Motion::Reduced => "settings-animations-reduced",
            Motion::Off => "settings-animations-off",
        }
    }
}

/// How fast animations and the table replay run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Slow,
    Normal,
    Fast,
    VeryFast,
}

impl Speed {
    pub const ALL: [Speed; 4] = [Speed::Slow, Speed::Normal, Speed::Fast, Speed::VeryFast];

    /// The stored setting value (unknown or unset = normal).
    pub fn from_setting(s: Option<&str>) -> Speed {
        match s {
            Some("slow") => Speed::Slow,
            Some("fast") => Speed::Fast,
            Some("very_fast") => Speed::VeryFast,
            _ => Speed::Normal,
        }
    }

    pub fn setting(self) -> &'static str {
        match self {
            Speed::Slow => "slow",
            Speed::Normal => "normal",
            Speed::Fast => "fast",
            Speed::VeryFast => "very_fast",
        }
    }

    /// Fluent id of the option's label.
    pub fn label_key(self) -> &'static str {
        match self {
            Speed::Slow => "settings-animation-speed-slow",
            Speed::Normal => "settings-animation-speed-normal",
            Speed::Fast => "settings-animation-speed-fast",
            Speed::VeryFast => "settings-animation-speed-very-fast",
        }
    }

    /// Duration multiplier.
    fn factor(self) -> f32 {
        match self {
            Speed::Slow => 1.6,
            Speed::Normal => 1.0,
            Speed::Fast => 0.6,
            Speed::VeryFast => 0.3,
        }
    }

    /// `d` at this speed.
    pub fn scale(self, d: Duration) -> Duration {
        d.mul_f32(self.factor())
    }
}

thread_local! {
    // Process-wide preferences read by free drawing functions (`felt`),
    // which have no `App` to look a global up in.
    static LEVEL: Cell<Motion> = const { Cell::new(Motion::On) };
    static SPEED: Cell<Speed> = const { Cell::new(Speed::Normal) };
}

/// The current level.
pub fn level() -> Motion {
    LEVEL.with(Cell::get)
}

/// Sets the level (startup and the settings control).
pub fn set(m: Motion) {
    LEVEL.with(|l| l.set(m));
}

/// The current speed.
pub fn speed() -> Speed {
    SPEED.with(Cell::get)
}

/// Sets the speed (startup and the settings control).
pub fn set_speed(s: Speed) {
    SPEED.with(|c| c.set(s));
}

/// `d` at the current speed (animations and the table replay).
pub fn scaled(d: Duration) -> Duration {
    speed().scale(d)
}

/// Fades take this long (at normal speed).
const FADE: Duration = Duration::from_millis(180);
/// Moves (deal, chips) take this long (at normal speed).
const MOVE: Duration = Duration::from_millis(260);

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// `el` fading in (on and reduced).
pub fn fade_in<E>(el: E, id: impl Into<ElementId>) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if level() == Motion::Off {
        return el.into_any_element();
    }
    el.with_animation(
        id,
        Animation::new(scaled(FADE)).with_easing(ease_out),
        |e, d| e.opacity(d),
    )
    .into_any_element()
}

/// An in-flow element (a card) dropping into place from `dy` pixels above
/// while it fades in (on); fade only when reduced.
pub fn deal_in<E>(el: E, id: impl Into<ElementId>, dy: f32) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    match level() {
        Motion::Off => el.into_any_element(),
        Motion::Reduced => fade_in(el, id),
        Motion::On => el
            .with_animation(
                id,
                Animation::new(scaled(MOVE)).with_easing(ease_out),
                move |e, d| e.relative().top(px(-dy * (1. - d))).opacity(d),
            )
            .into_any_element(),
    }
}

/// An absolutely positioned element sliding from `from` to `to` (its
/// left/top, in pixels) while it fades in (on); placed at `to` and faded
/// when reduced; placed when off.
pub fn slide<E>(el: E, id: impl Into<ElementId>, from: (f32, f32), to: (f32, f32)) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    match level() {
        Motion::Off => el.left(px(to.0)).top(px(to.1)).into_any_element(),
        Motion::Reduced => fade_in(el.left(px(to.0)).top(px(to.1)), id),
        Motion::On => el
            .with_animation(
                id,
                Animation::new(scaled(MOVE)).with_easing(ease_out),
                move |e, d| {
                    let x = from.0 + (to.0 - from.0) * d;
                    let y = from.1 + (to.1 - from.1) * d;
                    e.left(px(x)).top(px(y)).opacity(0.3 + 0.7 * d)
                },
            )
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_round_trips() {
        for m in Motion::ALL {
            assert_eq!(Motion::from_setting(Some(m.setting())), m);
        }
        assert_eq!(Motion::from_setting(None), Motion::On);
        assert_eq!(Motion::from_setting(Some("bogus")), Motion::On);
    }

    #[test]
    fn speed_round_trips_and_scales() {
        for s in Speed::ALL {
            assert_eq!(Speed::from_setting(Some(s.setting())), s);
        }
        assert_eq!(Speed::from_setting(None), Speed::Normal);
        assert_eq!(Speed::from_setting(Some("bogus")), Speed::Normal);
        let d = Duration::from_millis(1000);
        assert_eq!(Speed::Normal.scale(d), d);
        // Slower to faster: strictly shorter durations.
        let ds: Vec<_> = Speed::ALL.iter().map(|s| s.scale(d)).collect();
        assert!(ds.windows(2).all(|w| w[0] > w[1]));
    }
}
