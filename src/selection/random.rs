//! Uniform random selection without replacement.

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use super::{DrawRequest, SelectionError, Selector};
use crate::domain::Theorem;

/// Draws theorems uniformly at random, each equally likely, without repeats.
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomSelector;

impl RandomSelector {
    pub fn new() -> Self {
        Self
    }
}

impl Selector for RandomSelector {
    fn draw(
        &self,
        pool: &[Theorem],
        request: &DrawRequest,
    ) -> Result<Vec<Theorem>, SelectionError> {
        // Validate the request against the pool before touching the RNG, so the
        // error messages are precise.
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

        // Shuffle indices and take the first `count`: simple, and uniform over
        // all size-`count` subsets.
        let mut indices: Vec<usize> = (0..pool.len()).collect();
        match request.seed {
            Some(seed) => indices.shuffle(&mut StdRng::seed_from_u64(seed)),
            None => indices.shuffle(&mut rand::thread_rng()),
        }

        Ok(indices
            .into_iter()
            .take(request.count)
            .map(|i| pool[i].clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn pool(n: usize) -> Vec<Theorem> {
        (0..n)
            .map(|i| Theorem::new("Subject", format!("Theorem {i}"), "$x$").unwrap())
            .collect()
    }

    #[test]
    fn zero_count_is_rejected() {
        let sel = RandomSelector::new();
        assert!(matches!(
            sel.draw(&pool(3), &DrawRequest::new(0)),
            Err(SelectionError::ZeroCount)
        ));
    }

    #[test]
    fn empty_pool_is_rejected() {
        let sel = RandomSelector::new();
        assert!(matches!(
            sel.draw(&pool(0), &DrawRequest::new(3)),
            Err(SelectionError::EmptyPool)
        ));
    }

    #[test]
    fn requesting_more_than_available_is_rejected() {
        let sel = RandomSelector::new();
        assert!(matches!(
            sel.draw(&pool(2), &DrawRequest::new(3)),
            Err(SelectionError::NotEnough {
                requested: 3,
                available: 2
            })
        ));
    }

    #[test]
    fn draws_exactly_count_distinct_theorems_from_the_pool() {
        let sel = RandomSelector::new();
        let p = pool(5);
        let drawn = sel.draw(&p, &DrawRequest::new(3)).unwrap();

        assert_eq!(drawn.len(), 3);
        // Distinct...
        let ids: HashSet<_> = drawn.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 3);
        // ...and every drawn theorem came from the pool.
        let pool_ids: HashSet<_> = p.iter().map(|t| t.id).collect();
        assert!(ids.is_subset(&pool_ids));
    }

    #[test]
    fn drawing_all_returns_the_whole_pool() {
        let sel = RandomSelector::new();
        let p = pool(4);
        let drawn = sel.draw(&p, &DrawRequest::new(4)).unwrap();
        let drawn_ids: HashSet<_> = drawn.iter().map(|t| t.id).collect();
        let pool_ids: HashSet<_> = p.iter().map(|t| t.id).collect();
        assert_eq!(drawn_ids, pool_ids);
    }

    #[test]
    fn same_seed_yields_identical_draws() {
        let sel = RandomSelector::new();
        let p = pool(10);
        let a = sel.draw(&p, &DrawRequest::seeded(3, 42)).unwrap();
        let b = sel.draw(&p, &DrawRequest::seeded(3, 42)).unwrap();
        let ids_a: Vec<_> = a.iter().map(|t| t.id).collect();
        let ids_b: Vec<_> = b.iter().map(|t| t.id).collect();
        assert_eq!(ids_a, ids_b);
    }
}
