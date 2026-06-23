//! Use-case: add a theorem to the library.

use super::{App, AppError};
use crate::domain::Theorem;

/// Raw, unvalidated inputs for adding a theorem. Validation happens in the
/// domain layer when the [`Theorem`] is constructed.
#[derive(Debug, Clone)]
pub struct AddRequest {
    pub subject: String,
    pub name: String,
    pub content: String,
}

impl App {
    /// Validate the inputs into a [`Theorem`], persist it, and return it (so the
    /// caller can report its id and a summary).
    pub fn add(&mut self, request: AddRequest) -> Result<Theorem, AppError> {
        let theorem = Theorem::new(request.subject, request.name, request.content)?;
        self.repo.add(theorem.clone())?;
        Ok(theorem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn add_persists_a_valid_theorem() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());

        let theorem = app
            .add(AddRequest {
                subject: "Analysis".into(),
                name: "MCT".into(),
                content: "$\\lim$".into(),
            })
            .unwrap();

        let stored = app.repo.all().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0], theorem);
        assert_eq!(stored[0].subject.as_str(), "Analysis");
    }

    #[test]
    fn add_rejects_invalid_input_without_persisting() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());

        let err = app
            .add(AddRequest {
                subject: "".into(),
                name: "MCT".into(),
                content: "$x$".into(),
            })
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
        assert_eq!(app.repo.all().unwrap().len(), 0);
    }
}
