//! Deterministic, tier-by-tier effect-policy oracle.
//!
//! In place of GPU pixel goldens (which need a device and human visual
//! approval), this pins the *approved policy* of every quality tier: the
//! exact bounded limits and capability flags each tier commits to, and
//! the cross-tier invariants that keep degradation coherent (`phase.md`
//! Phase 12 exit gate: "tier switching is stable; Efficient and Recovery
//! remain readable and coherent; no effect can trigger unbounded texture
//! allocation"). A change to any tier's contract must update this oracle
//! deliberately, exactly as re-approving a golden image would.

use aura_graphics::quality::Tier;

const ALL_TIERS: [Tier; 4] = [Tier::Ultra, Tier::Balanced, Tier::Efficient, Tier::Recovery];

#[test]
fn every_tier_pins_its_approved_policy() {
    let ultra = Tier::Ultra.policy();
    assert_eq!(ultra.max_blur_radius, 32.0);
    assert!(ultra.backdrop_blur && ultra.noise);

    let balanced = Tier::Balanced.policy();
    assert_eq!(balanced.max_blur_radius, 20.0);
    assert!(balanced.backdrop_blur && balanced.noise);

    let efficient = Tier::Efficient.policy();
    assert_eq!(efficient.max_blur_radius, 8.0);
    assert!(!efficient.backdrop_blur && !efficient.noise);

    let recovery = Tier::Recovery.policy();
    assert_eq!(recovery.max_blur_radius, 0.0);
    assert!(!recovery.backdrop_blur && !recovery.noise);
}

#[test]
fn degradation_is_monotonic_across_every_adjacent_tier() {
    for pair in ALL_TIERS.windows(2) {
        let higher = pair[0].policy();
        let lower = pair[1].policy();

        assert!(
            lower.max_blur_radius <= higher.max_blur_radius,
            "{:?} allows a larger blur than {:?}",
            pair[1],
            pair[0]
        );
        assert!(lower.max_effect_area <= higher.max_effect_area);
        assert!(lower.texture_budget_bytes <= higher.texture_budget_bytes);
    }
}

#[test]
fn no_capability_re_enables_as_the_tier_degrades() {
    // Backdrop blur and noise, once disabled by a lower tier, must stay
    // disabled for every lower tier still -- otherwise a degraded tier
    // could paradoxically be more expensive than a richer one.
    let mut backdrop_seen_off = false;
    let mut noise_seen_off = false;

    for tier in ALL_TIERS {
        let policy = tier.policy();
        if backdrop_seen_off {
            assert!(!policy.backdrop_blur, "{tier:?} re-enabled backdrop blur");
        }
        if noise_seen_off {
            assert!(!policy.noise, "{tier:?} re-enabled noise");
        }
        backdrop_seen_off |= !policy.backdrop_blur;
        noise_seen_off |= !policy.noise;
    }
}

#[test]
fn every_tier_keeps_a_nonzero_texture_budget_so_the_shell_can_render() {
    // Even Recovery must be able to allocate *some* intermediate, or it
    // could not render the shell at all -- "Recovery tier must render the
    // shell without expensive effects", not "cannot render".
    for tier in ALL_TIERS {
        assert!(
            tier.policy().texture_budget_bytes > 0,
            "{tier:?} has a zero texture budget and could not render"
        );
    }
}

#[test]
fn recovery_forbids_every_bounds_expanding_effect() {
    // Recovery's blur radius is zero, so no blur can expand damage
    // bounds: the shell renders with no expensive sampling at all.
    let recovery = Tier::Recovery.policy();
    assert_eq!(recovery.max_blur_radius, 0.0);
    assert_eq!(recovery.max_effect_area, 0.0);
}
