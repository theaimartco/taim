//! TAIM-native deterministic bounded flex-space planning.
//!
//! Reserve minima first, share shortages proportionally, then distribute
//! remaining space by weight while removing saturated items. Sorting the
//! cap thresholds makes allocation O(n log n) with O(n) storage.
//! This planner does not measure widgets or borrow UI tree state.

use crate::length::{Bounds, Constraint, Sizing};
use crate::Length;

#[derive(Debug, Clone, Copy)]
pub(super) struct Request {
    pub(super) weight: u16,
    pub(super) minimum: f32,
    pub(super) maximum: f32,
    intrinsic_cap: bool,
}

impl Request {
    pub(super) fn from_length(length: Length) -> Self {
        let weight = length.fill_factor();
        let (min, max, intrinsic_cap) = match length {
            Length::Bounded { sizing: Sizing::Fill(_), bounds: Bounds::Min(min) }
                => (min, f32::INFINITY, false),
            Length::Bounded { sizing: Sizing::Fill(_), bounds: Bounds::Max(max) }
                => (0.0, max, false),
            Length::Bounded { sizing: Sizing::Fill(_), bounds: Bounds::Both { min, max } }
                => (min, max, false),
            Length::Fluid(Constraint::Min(min)) => (min, f32::INFINITY, false),
            Length::Fluid(Constraint::Max) => (0.0, f32::INFINITY, true),
            _ => (0.0, f32::INFINITY, false),
        };
        let minimum = if min.is_finite() { min.max(0.0) } else { 0.0 };
        let maximum = if max.is_nan() { minimum } else { max.max(minimum) };
        Self { weight, minimum, maximum, intrinsic_cap: weight != 0 && intrinsic_cap }
    }

    pub(super) fn intrinsic_cap(self) -> bool {
        self.intrinsic_cap
    }

    pub(super) fn suppress(&mut self) {
        self.weight = 0;
        self.minimum = 0.0;
        self.maximum = 0.0;
        self.intrinsic_cap = false;
    }
}

/// Allocate target extents; no child layout is executed in this function.
///
/// Finite input gives finite nonnegative results. Infinite input retains
/// unbounded layout semantics (callers must offer loose child constraints).
pub(super) fn allocate(available: f32, requests: &[Request]) -> Vec<f32> {
    let mut grants = vec![0.0_f32; requests.len()];
    if available.is_nan() || available <= 0.0 {
        return grants;
    }
    if available.is_infinite() {
        for (grant, req) in grants.iter_mut().zip(requests) {
            if req.weight > 0 {
                *grant = req.maximum;
            }
        }
        return grants;
    }

    let budget = f64::from(available);
    let mut floors = vec![0.0_f64; requests.len()];
    let mut floor_sum = 0.0_f64;
    for (i, req) in requests.iter().enumerate() {
        if req.weight > 0 {
            floors[i] = f64::from(req.minimum);
            floor_sum += floors[i];
        }
    }
    if floor_sum >= budget {
        let fraction = if floor_sum > 0.0 { budget / floor_sum } else { 0.0 };
        for (i, floor) in floors.iter().enumerate() {
            grants[i] = (floor * fraction) as f32;
        }
        return grants;
    }

    let mut events: Vec<(f64, usize)> = Vec::with_capacity(requests.len());
    let mut total_weight = 0.0_f64;
    for (i, req) in requests.iter().enumerate() {
        if req.weight == 0 {
            continue;
        }
        let weight = f64::from(req.weight);
        let room = (f64::from(req.maximum) - floors[i]).max(0.0);
        if room <= 0.0 {
            continue;
        }
        total_weight += weight;
        if room.is_finite() {
            events.push((room / weight, i));
        }
    }
    events.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

    let mut remaining = budget - floor_sum;
    let mut level = 0.0_f64;
    let mut previous = 0.0_f64;
    if total_weight > 0.0 {
        for (threshold, i) in events {
            let next = threshold.max(previous);
            let cost = (next - previous) * total_weight;
            if remaining <= cost {
                level = previous + remaining / total_weight;
                remaining = 0.0;
                break;
            }
            remaining -= cost;
            previous = next;
            total_weight -= f64::from(requests[i].weight);
            level = previous;
        }
        if remaining > 0.0 && total_weight > 0.0 {
            level += remaining / total_weight;
        }
    }

    for (i, req) in requests.iter().enumerate() {
        if req.weight != 0 {
            let desired = (floors[i] + level * f64::from(req.weight))
                .min(f64::from(req.maximum));
            grants[i] = desired.min(budget).max(0.0) as f32;
        }
    }
    grants
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(weight: u16, min: f32, max: f32) -> Request {
        Request { weight, minimum: min, maximum: max, intrinsic_cap: false }
    }

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() <= 0.001, "{actual} != {expected}");
    }

    #[test]
    fn weights_determine_stable_share() {
        let req = [demand(1, 0.0, f32::INFINITY), demand(3, 0.0, f32::INFINITY)];
        let result = allocate(100.0, &req);
        close(result[0], 25.0);
        close(result[1], 75.0);
        assert_eq!(result, allocate(100.0, &req));
    }

    #[test]
    fn saturation_redistributes_budget() {
        let req = [demand(1, 0.0, 20.0), demand(1, 0.0, f32::INFINITY)];
        assert_eq!(allocate(100.0, &req), vec![20.0, 80.0]);
    }

    #[test]
    fn minimum_reserves_space_and_gets_fair_surplus() {
        let req = [demand(1, 70.0, f32::INFINITY), demand(1, 0.0, f32::INFINITY)];
        assert_eq!(allocate(100.0, &req), vec![85.0, 15.0]);
    }

    #[test]
    fn oversubscribed_minima_share_shortage() {
        let req = [demand(1, 80.0, f32::INFINITY), demand(1, 80.0, f32::INFINITY)];
        assert_eq!(allocate(100.0, &req), vec![50.0, 50.0]);
    }

    #[test]
    fn caps_and_weights_work_for_multiple_items() {
        let req = [
            demand(1, 0.0, 10.0),
            demand(2, 0.0, 50.0),
            demand(1, 0.0, f32::INFINITY),
        ];
        assert_eq!(allocate(100.0, &req), vec![10.0, 50.0, 40.0]);
    }

    #[test]
    fn zero_weight_zero_space_and_nonfinite_budget_are_safe() {
        let req = [demand(0, 20.0, 40.0), demand(1, 0.0, 100.0)];
        assert_eq!(allocate(30.0, &req), vec![0.0, 30.0]);
        assert_eq!(allocate(0.0, &req), vec![0.0, 0.0]);
        assert_eq!(allocate(f32::NAN, &req), vec![0.0, 0.0]);
    }

    #[test]
    fn infinite_container_preserves_unbounded_child_constraint() {
        let req = [demand(1, 0.0, 20.0), demand(1, 0.0, f32::INFINITY)];
        assert_eq!(allocate(f32::INFINITY, &req), vec![20.0, f32::INFINITY]);
    }

    #[test]
    fn length_bounds_and_intrinsic_caps_are_distinct() {
        let capped = Request::from_length(Length::Fill.max(22.0));
        assert_eq!(capped.maximum, 22.0);
        let floor = Request::from_length(Length::Fill.min(15.0));
        assert_eq!(floor.minimum, 15.0);
        let mut intrinsic = Request::from_length(Length::Fluid(Constraint::Max));
        assert!(intrinsic.intrinsic_cap());
        intrinsic.suppress();
        assert_eq!(allocate(100.0, &[intrinsic]), vec![0.0]);
    }

    #[test]
    fn malformed_bounds_cannot_poison_results() {
        let req = [
            demand(1, 0.0, f32::INFINITY),
            Request::from_length(Length::Bounded {
                bounds: Bounds::Both { min: f32::NAN, max: f32::NAN },
                sizing: Sizing::Fill(1),
            }),
        ];
        assert!(allocate(45.0, &req).iter().all(|x| x.is_finite() && *x >= 0.0));
    }
}
