/// Produces a finite, forward-moving slider step.
#[inline]
pub(crate) fn normalize_step(step: f64) -> f64 {
    if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    }
}

/// Snaps a normalized track position to the range's step grid.
#[inline]
pub(crate) fn snap(start: f64, end: f64, ratio: f64, step: f64) -> f64 {
    if !start.is_finite() || !end.is_finite() || end <= start {
        return start;
    }

    let step = normalize_step(step);
    let steps = (ratio.clamp(0.0, 1.0) * (end - start) / step).round();

    (start + steps * step).clamp(start, end)
}

/// Advances a value on a step grid anchored at the range start.
#[inline]
pub(crate) fn advance(start: f64, end: f64, value: f64, step: f64, direction: f64) -> f64 {
    if !start.is_finite() || !end.is_finite() || end <= start {
        return start;
    }

    let step = normalize_step(step);
    let index = ((value - start) / step).round() + direction.signum();

    (start + index * step).clamp(start, end)
}
