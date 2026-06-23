//! Selection layer: choosing which theorems to draw, behind the [`Selector`]
//! trait.
//!
//! This is the seam where future strategies (spaced repetition,
//! recency-avoidance, per-subject quotas) will live — each as another
//! `Selector` the app can hold without caring which.

mod error;
mod forgetting;
mod random;

pub use error::SelectionError;
pub use forgetting::ForgettingCurveSelector;
pub use random::RandomSelector;

use chrono::{DateTime, Utc};

use crate::domain::Theorem;

/// What to draw: how many theorems, an optional seed for reproducibility, and
/// the time the draw is evaluated at (the forgetting-curve weighting measures
/// recency against it; uniform selection ignores it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawRequest {
    /// Number of theorems to draw.
    pub count: usize,
    /// If set, makes the draw deterministic (same pool state + seed → same
    /// result). Useful for tests and for regenerating an identical practice sheet.
    pub seed: Option<u64>,
    /// "Now" for recency: how long ago each theorem was last drawn is measured
    /// against this. Passed in (not read from a clock) to keep selectors pure.
    pub now: DateTime<Utc>,
}

impl DrawRequest {
    /// A request to draw `count` theorems with fresh randomness, evaluated now.
    pub fn new(count: usize) -> Self {
        Self {
            count,
            seed: None,
            now: Utc::now(),
        }
    }

    /// A request to draw `count` theorems deterministically from `seed`,
    /// evaluated now.
    pub fn seeded(count: usize, seed: u64) -> Self {
        Self {
            count,
            seed: Some(seed),
            now: Utc::now(),
        }
    }
}

/// A strategy for choosing theorems to practice from a pool.
pub trait Selector {
    /// Draw theorems from `pool` according to `request`.
    ///
    /// Implementations must return exactly `request.count` theorems on success,
    /// or a [`SelectionError`] explaining why they could not.
    fn draw(
        &self,
        pool: &[Theorem],
        request: &DrawRequest,
    ) -> Result<Vec<Theorem>, SelectionError>;
}
