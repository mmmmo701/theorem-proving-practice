//! Use-case: list and inspect stored theorems.

use super::{App, AppError};
use crate::domain::Theorem;

impl App {
    /// Every stored theorem, oldest first (stable, predictable order).
    pub fn list(&self) -> Result<Vec<Theorem>, AppError> {
        let mut all = self.repo.all()?;
        all.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        Ok(all)
    }

    /// Theorems whose id (hyphenated, lowercase form) starts with `prefix`,
    /// oldest first.
    ///
    /// Returns the raw matches; the caller decides how to treat zero or many
    /// (so identifier-resolution policy lives in the front-end, not here).
    pub fn find_by_id_prefix(&self, prefix: &str) -> Result<Vec<Theorem>, AppError> {
        let prefix = prefix.trim().to_lowercase();
        let mut matches: Vec<Theorem> = self
            .repo
            .all()?
            .into_iter()
            .filter(|t| t.id.to_string().starts_with(&prefix))
            .collect();
        matches.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AddRequest;
    use crate::config::Config;
    use crate::storage::JsonStore;
    use tempfile::tempdir;

    fn app_in(dir: &std::path::Path) -> App {
        let config = Config {
            data_dir: dir.to_path_buf(),
            ..Config::default()
        };
        let repo = JsonStore::new(config.store_path());
        App::new(config, Box::new(repo))
    }

    fn add(app: &mut App, name: &str) -> Theorem {
        app.add(AddRequest {
            subject: "Subject".into(),
            name: name.into(),
            content: "$x$".into(),
        })
        .unwrap()
    }

    #[test]
    fn list_is_empty_for_a_fresh_library() {
        let dir = tempdir().unwrap();
        let app = app_in(dir.path());
        assert!(app.list().unwrap().is_empty());
    }

    #[test]
    fn list_returns_all_oldest_first() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let first = add(&mut app, "first");
        let second = add(&mut app, "second");

        let listed = app.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[1].id, second.id);
    }

    #[test]
    fn find_by_full_id_returns_exactly_that_theorem() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let t = add(&mut app, "only");
        let found = app.find_by_id_prefix(&t.id.to_string()).unwrap();
        assert_eq!(found, vec![t]);
    }

    #[test]
    fn find_by_unknown_prefix_returns_nothing() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        add(&mut app, "only");
        assert!(app.find_by_id_prefix("zzzzzzzz").unwrap().is_empty());
    }
}
