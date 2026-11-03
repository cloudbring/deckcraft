//! Time easing: cubic-Bezier curves, acceleration / deceleration fractions and bounce.

use crate::clampf;

fn cubic(control1: f64, control2: f64, t: f64) -> f64 {
    let remaining = 1.0 - t;
    3.0 * remaining * remaining * t * control1 + 3.0 * remaining * t * t * control2 + t * t * t
}

fn cubic_derivative(control1: f64, control2: f64, t: f64) -> f64 {
    let remaining = 1.0 - t;
    3.0 * remaining * remaining * control1 + 6.0 * remaining * t * (control2 - control1) + 3.0 * t * t * (1.0 - control2)
}

fn cubic_time_for_x(x1: f64, x2: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }

    const SAMPLES: usize = 10;
    let mut lower = 0.0;
    let mut upper = 1.0;
    let mut lower_x = 0.0;
    let mut upper_x = 1.0;
    for index in 1..=SAMPLES {
        let sample = index as f64 / SAMPLES as f64;
        let sample_x = cubic(x1, x2, sample);
        if x <= sample_x {
            upper = sample;
            upper_x = sample_x;
            break;
        }
        lower = sample;
        lower_x = sample_x;
    }

    let fraction = if upper_x > lower_x { (x - lower_x) / (upper_x - lower_x) } else { 0.5 };
    let mut t = lower + fraction * (upper - lower);
    for _ in 0..8 {
        let residual = cubic(x1, x2, t) - x;
        if residual == 0.0 {
            return t;
        }
        if residual < 0.0 {
            lower = t;
        } else {
            upper = t;
        }
        let derivative = cubic_derivative(x1, x2, t);
        if derivative <= 1e-12 {
            break;
        }
        let step = residual / derivative;
        if step.abs() <= 1e-12 {
            return t;
        }
        let next = t - step;
        if next <= lower || next >= upper {
            break;
        }
        t = next;
    }
    for _ in 0..48 {
        let midpoint = (lower + upper) * 0.5;
        if cubic(x1, x2, midpoint) < x {
            lower = midpoint;
        } else {
            upper = midpoint;
        }
        if upper - lower <= 1e-12 {
            break;
        }
    }
    (lower + upper) * 0.5
}

/// Evaluate a CSS cubic-Bezier time curve at `x` with handles `(x1, y1)` and `(x2, y2)`.
/// The x handles are clamped to [0, 1]; the y handles may overshoot. Samples bracket Newton
/// iterations, with a bisection fallback. The absolute x residual is at most 1e-7, and the
/// parameter interval is narrowed to 1e-12 to control y error near flat x derivatives.
/// Non-finite inputs return clamped linear progress.
pub fn cubic_bezier_y_for_x(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let progress = clampf(x, 0.0, 1.0, 0.0);
    if ![x1, y1, x2, y2, x].iter().all(|value| value.is_finite()) {
        return progress;
    }
    let t = cubic_time_for_x(x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0), progress);
    let result = cubic(y1, y2, t);
    if result.is_finite() { result } else { progress }
}

/// Map linear progress `p` (0..1) through PowerPoint-style easing: `accel` and `decel` are the
/// fractions of the duration spent speeding up and slowing down (velocity ramps linearly);
/// `bounce` (0..1) makes the last part of the motion bounce against the end.
pub fn ease(p: f64, accel: f64, decel: f64, bounce: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    let mut a = clampf(accel, 0.0, 1.0, 0.0);
    let mut d = clampf(decel, 0.0, 1.0, 0.0);
    if a + d > 1.0 {
        let s = a + d;
        a /= s;
        d /= s;
    }
    let v = 2.0 / (2.0 - a - d);
    let mut y = if a > 0.0 && p < a {
        v * p * p / (2.0 * a)
    } else if d > 0.0 && p > 1.0 - d {
        let q = 1.0 - p;
        1.0 - v * q * q / (2.0 * d)
    } else {
        v * a / 2.0 + v * (p - a)
    };
    let b = clampf(bounce, 0.0, 1.0, 0.0);
    if b > 0.0 {
        // Reach the end early, then bounce back with decaying hops over the last `b` fraction.
        let hit = 1.0 - b;
        if p < hit {
            let q = if hit > 0.0 { p / hit } else { 1.0 };
            y = q * q;
        } else {
            let s = (p - hit) / b;
            y = 1.0 - bounce_hops(s) * 0.25;
        }
    }
    clampf(y, -1.0, 2.0, 0.0)
}

/// Decaying hops for s in 0..1: 0 at the ends of each hop, peaks shrinking.
fn bounce_hops(s: f64) -> f64 {
    let s = clampf(s, 0.0, 1.0, 1.0);
    let hops = 3.0;
    let k = (s * hops).floor().min(hops - 1.0);
    let local = s * hops - k;
    let amp = 0.5f64.powf(k);
    amp * (local * std::f64::consts::PI).sin()
}

/// Classic bounce-out: falls to 1 and bounces with decaying height.
pub(crate) fn bounce_out(p: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    let n = 7.5625;
    let d = 2.75;
    let y = if p < 1.0 / d {
        n * p * p
    } else if p < 2.0 / d {
        let q = p - 1.5 / d;
        n * q * q + 0.75
    } else if p < 2.5 / d {
        let q = p - 2.25 / d;
        n * q * q + 0.9375
    } else {
        let q = p - 2.625 / d;
        n * q * q + 0.984375
    };
    y.min(1.0)
}

/// Smooth ease-in-out (cubic smoothstep).
pub(crate) fn smooth(p: f64) -> f64 {
    let p = clampf(p, 0.0, 1.0, 0.0);
    p * p * (3.0 - 2.0 * p)
}
