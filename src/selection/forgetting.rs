//! Time-aware forgetting-curve selection.
//!
//! Each theorem gets a *selection weight* from how often and how recently it has
//! been drawn: well-practiced, recently-seen theorems are unlikely; neglected or
//! never-seen ones rise to the top. The model and its constants
//! (`BASE_STABILITY_DAYS`, `GROWTH`, `WEIGHT_FLOOR`) are documented inline below.

use chrono::{DateTime, Utc};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

use super::{DrawRequest, SelectionError, Selector};
use crate::domain::Theorem;

/// Half-life (days) of recall after a theorem's *first* draw.
const BASE_STABILITY_DAYS: f64 = 1.0;
/// Stability multiplier gained per additional draw (so stability is
/// `BASE_STABILITY_DAYS * GROWTH^(draws-1)`).
const GROWTH: f64 = 2.0;
/// Floor on the selection weight, so every theorem keeps a small chance of being
/// drawn and nothing becomes permanently unreachable.
const WEIGHT_FLOOR: f64 = 0.02;

/// Draws theorems weighted against the forgetting curve, without replacement.
#[derive(Debug, Default, Clone, Copy)]
pub struct ForgettingCurveSelector;

impl ForgettingCurveSelector {
    pub fn new() -> Self {
        Self
    }
}

impl Selector for ForgettingCurveSelector {
    fn draw(
        &self,
        pool: &[Theorem],
        request: &DrawRequest,
    ) -> Result<Vec<Theorem>, SelectionError> {
        // Same up-front validation as RandomSelector, for identical error
        // behavior.
        if request.count == 0 {
            return Err(SelectionError::ZeroCount);
        }
        if pool.is_empty() {
            return Err(SelectionError::EmptyPool);
        }
        if request.count > pool.len() {
            return Err(SelectionError::NotEnough {
                requested: request.count,
                available: pool.len(),
            });
        }

        // Weighted sampling without replacement (Efraimidis–Spirakis): give each
        // item the key ln(u)/w with u ∈ (0,1], then take the largest `count`
        // keys. Larger weight ⇒ key nearer 0 ⇒ more likely chosen.
        let mut keyed: Vec<(f64, usize)> = match request.seed {
            Some(seed) => sample_keys(pool, request.now, &mut StdRng::seed_from_u64(seed)),
            None => sample_keys(pool, request.now, &mut rand::thread_rng()),
        };

        // Sort by key descending; partial_cmp is fine (keys are finite).
        keyed.sort_by(|a, b| b.0.total_cmp(&a.0));

        Ok(keyed
            .into_iter()
            .take(request.count)
            .map(|(_, i)| pool[i].clone())
            .collect())
    }
}

/// Compute the Efraimidis–Spirakis key for every theorem in the pool.
fn sample_keys<R: Rng>(pool: &[Theorem], now: DateTime<Utc>, rng: &mut R) -> Vec<(f64, usize)> {
    pool.iter()
        .enumerate()
        .map(|(i, t)| {
            // u ∈ (0, 1]: `random::<f64>()` is [0,1), so 1.0 - it avoids ln(0).
            let u = 1.0 - rng.r#gen::<f64>();
            let key = u.ln() / weight(t, now);
            (key, i)
        })
        .collect()
}

/// Selection weight for one theorem at time `now`, in `(0, 1]`. Higher means
/// more likely to be drawn (lower recall ⇒ needs practice).
fn weight(theorem: &Theorem, now: DateTime<Utc>) -> f64 {
    let Some(last) = theorem.last_drawn_at else {
        // Never drawn: maximum priority.
        return 1.0;
    };

    // Drawn at least once (last_drawn_at is set); guard a stray count of 0.
    let draws = theorem.draw_count.max(1);
    let stability = BASE_STABILITY_DAYS * GROWTH.powi((draws - 1) as i32);

    // Days elapsed, clamped at 0 so clock skew / a future stamp can't go negative.
    let delta_days = ((now - last).num_seconds() as f64 / 86_400.0).max(0.0);
    let recall = (-delta_days / stability).exp();

    (1.0 - recall).max(WEIGHT_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    use std::collections::HashSet;

    fn at(days_ago: i64, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        Some(now - Duration::days(days_ago))
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 18, 12, 0, 0).unwrap()
    }

    /// A theorem with controlled draw stats (fields are public).
    fn drawn(name: &str, count: u32, last: Option<DateTime<Utc>>) -> Theorem {
        let mut t = Theorem::new("Subject", name, "$x$").unwrap();
        t.draw_count = count;
        t.last_drawn_at = last;
        t
    }

    #[test]
    fn never_drawn_has_maximum_weight() {
        let t = drawn("new", 0, None);
        assert_eq!(weight(&t, now()), 1.0);
    }

    #[test]
    fn recently_well_practiced_weighs_less_than_neglected() {
        let fresh = drawn("fresh", 0, None);
        let practiced_today = drawn("hot", 5, at(0, now()));
        let practiced_long_ago = drawn("cold", 2, at(7, now()));

        let w_fresh = weight(&fresh, now());
        let w_hot = weight(&practiced_today, now());
        let w_cold = weight(&practiced_long_ago, now());

        assert!(w_hot < w_cold, "{w_hot} !< {w_cold}");
        assert!(w_cold < w_fresh, "{w_cold} !< {w_fresh}");
    }

    #[test]
    fn weight_recovers_as_time_passes() {
        // Same draw count, more elapsed time ⇒ higher weight (the headline case).
        let recent = drawn("a", 5, at(1, now()));
        let stale = drawn("b", 5, at(30, now()));
        assert!(weight(&stale, now()) > weight(&recent, now()));
    }

    #[test]
    fn weight_never_drops_below_the_floor() {
        // Drawn many times, just now: recall ≈ 1, but the floor keeps it alive.
        let t = drawn("saturated", 20, at(0, now()));
        let w = weight(&t, now());
        assert!(w >= WEIGHT_FLOOR);
        assert!((w - WEIGHT_FLOOR).abs() < 1e-9, "expected floor, got {w}");
    }

    fn pool(n: usize) -> Vec<Theorem> {
        (0..n).map(|i| drawn(&format!("T{i}"), 0, None)).collect()
    }

    fn request(count: usize) -> DrawRequest {
        DrawRequest {
            count,
            seed: Some(42),
            now: now(),
        }
    }

    #[test]
    fn zero_count_is_rejected() {
        let sel = ForgettingCurveSelector::new();
        assert!(matches!(
            sel.draw(&pool(3), &request(0)),
            Err(SelectionError::ZeroCount)
        ));
    }

    #[test]
    fn empty_pool_is_rejected() {
        let sel = ForgettingCurveSelector::new();
        assert!(matches!(
            sel.draw(&pool(0), &request(3)),
            Err(SelectionError::EmptyPool)
        ));
    }

    #[test]
    fn requesting_more_than_available_is_rejected() {
        let sel = ForgettingCurveSelector::new();
        assert!(matches!(
            sel.draw(&pool(2), &request(3)),
            Err(SelectionError::NotEnough {
                requested: 3,
                available: 2
            })
        ));
    }

    #[test]
    fn draws_exactly_count_distinct_from_the_pool() {
        let sel = ForgettingCurveSelector::new();
        let p = pool(5);
        let drawn = sel.draw(&p, &request(3)).unwrap();
        assert_eq!(drawn.len(), 3);
        let ids: HashSet<_> = drawn.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 3);
        let pool_ids: HashSet<_> = p.iter().map(|t| t.id).collect();
        assert!(ids.is_subset(&pool_ids));
    }

    #[test]
    fn same_seed_yields_identical_draws() {
        let sel = ForgettingCurveSelector::new();
        let p = pool(10);
        let a: Vec<_> = sel.draw(&p, &request(3)).unwrap().iter().map(|t| t.id).collect();
        let b: Vec<_> = sel.draw(&p, &request(3)).unwrap().iter().map(|t| t.id).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn a_dominant_weight_is_chosen_over_saturated_peers() {
        // One never-drawn (w = 1) among nine saturated (w = floor): the single
        // draw should pick the dominant one. Robust across seeds in practice.
        let sel = ForgettingCurveSelector::new();
        let mut p: Vec<Theorem> = (0..9).map(|i| drawn(&format!("hot{i}"), 20, at(0, now()))).collect();
        let target = drawn("fresh", 0, None);
        p.push(target.clone());

        let drawn_one = sel.draw(&p, &request(1)).unwrap();
        assert_eq!(drawn_one[0].id, target.id);
    }
}
