//! Use-case: delete a theorem from the library.

use super::{App, AppError};
use crate::domain::TheoremId;

impl App {
    /// Remove the theorem with the given id, returning `true` if one was removed
    /// and `false` if no theorem had that id.
    ///
    /// Resolving an id-prefix to a single id is a front-end concern (see
    /// [`App::find_by_id_prefix`]); this use-case acts on an exact id.
    pub fn delete(&mut self, id: &TheoremId) -> Result<bool, AppError> {
        Ok(self.repo.remove(id)?)
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

    fn add(app: &mut App, name: &str) -> TheoremId {
        app.add(AddRequest {
            subject: "Subject".into(),
            name: name.into(),
            content: "$x$".into(),
        })
        .unwrap()
        .id
    }

    #[test]
    fn delete_removes_only_the_named_theorem() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let keep = add(&mut app, "keep");
        let drop = add(&mut app, "drop");

        assert!(app.delete(&drop).unwrap());
        let remaining = app.list().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, keep);
    }

    #[test]
    fn delete_unknown_id_reports_false() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        add(&mut app, "only");
        assert!(!app.delete(&TheoremId::new()).unwrap());
        assert_eq!(app.list().unwrap().len(), 1);
    }
}
