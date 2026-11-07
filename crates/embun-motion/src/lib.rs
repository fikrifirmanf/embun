//! Springs for Embun. Every Embun animation is a spring, as in macOS 27.
//!
//! A spring is described the SwiftUI way: `response` is roughly how long one
//! oscillation takes (seconds), and `damping_fraction` is 1 for no overshoot
//! and lower for bounce.

use std::f32::consts::TAU;

/// How a value moves toward its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub response: f32,
    pub damping_fraction: f32,
}

impl Spring {
    /// No overshoot. The default for glass level, tint and appearance changes.
    pub const SMOOTH: Spring = Spring { response: 0.5, damping_fraction: 1.0 };
    /// A little overshoot. For shapes merging, splitting and moving.
    pub const SNAPPY: Spring = Spring { response: 0.35, damping_fraction: 0.85 };
    /// Visible bounce.
    pub const BOUNCY: Spring = Spring { response: 0.5, damping_fraction: 0.7 };

    fn stiffness(self) -> f32 {
        let w = TAU / self.response.max(1e-3);
        w * w
    }

    fn damping(self) -> f32 {
        4.0 * std::f32::consts::PI * self.damping_fraction.max(0.0) / self.response.max(1e-3)
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::SMOOTH
    }
}

/// A value animated by a spring. Call [`Animated::step`] once per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animated {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    pub spring: Spring,
}

/// Fixed simulation step, so results don't depend on the frame rate.
const SUBSTEP: f32 = 1.0 / 240.0;
/// Closer than this, with almost no velocity, the value snaps to the target.
const REST: f32 = 1e-4;

impl Animated {
    pub fn new(value: f32, spring: Spring) -> Self {
        Self { value, velocity: 0.0, target: value, spring }
    }

    /// Advances the simulation by `dt` seconds. Returns true while still moving.
    pub fn step(&mut self, dt: f32) -> bool {
        let (k, c) = (self.spring.stiffness(), self.spring.damping());
        let mut remaining = dt.clamp(0.0, 0.25);
        while remaining > 0.0 {
            let h = remaining.min(SUBSTEP);
            let accel = -k * (self.value - self.target) - c * self.velocity;
            self.velocity += accel * h; // semi-implicit Euler: velocity first
            self.value += self.velocity * h;
            remaining -= h;
        }
        if self.is_settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }

    pub fn is_settled(&self) -> bool {
        (self.value - self.target).abs() < REST && self.velocity.abs() < REST * 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spring: Spring, seconds: f32) -> (Animated, f32) {
        let mut a = Animated::new(0.0, spring);
        a.target = 1.0;
        let mut peak = 0.0f32;
        for _ in 0..(seconds * 60.0) as usize {
            a.step(1.0 / 60.0);
            peak = peak.max(a.value);
        }
        (a, peak)
    }

    #[test]
    fn smooth_spring_reaches_target_without_overshoot() {
        let (a, peak) = run(Spring::SMOOTH, 2.0);
        assert_eq!(a.value, 1.0);
        assert!(peak <= 1.0 + 1e-4, "overshot to {peak}");
    }

    #[test]
    fn bouncy_spring_overshoots_then_settles() {
        let (a, peak) = run(Spring::BOUNCY, 3.0);
        assert!(peak > 1.02, "expected overshoot, peak was {peak}");
        assert_eq!(a.value, 1.0);
    }

    #[test]
    fn result_does_not_depend_on_frame_rate() {
        let mut at_60 = Animated::new(0.0, Spring::SNAPPY);
        let mut at_120 = at_60;
        at_60.target = 1.0;
        at_120.target = 1.0;
        for _ in 0..12 {
            at_60.step(1.0 / 60.0);
        }
        for _ in 0..24 {
            at_120.step(1.0 / 120.0);
        }
        assert!((at_60.value - at_120.value).abs() < 1e-3, "{} vs {}", at_60.value, at_120.value);
    }

    #[test]
    fn step_reports_motion_until_settled() {
        let mut a = Animated::new(0.0, Spring::SMOOTH);
        assert!(!a.step(1.0 / 60.0), "a spring at rest on its target is not moving");
        a.target = 1.0;
        assert!(a.step(1.0 / 60.0));
    }

    #[test]
    fn huge_frame_gaps_stay_stable() {
        let mut a = Animated::new(0.0, Spring::BOUNCY);
        a.target = 1.0;
        a.step(10.0); // e.g. the tab was in the background
        assert!(a.value.is_finite() && a.value.abs() < 3.0, "value blew up: {}", a.value);
    }
}
