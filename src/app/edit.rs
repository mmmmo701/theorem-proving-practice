//! Use-case: edit an existing theorem's fields.

use super::{App, AppError};
use crate::domain::{LatexContent, Name, Subject, Theorem, TheoremId};

/// Raw, unvalidated replacement values for an edit. `None` leaves that field
/// unchanged. Validation happens in the domain layer when the newtypes are
/// constructed.
#[derive(Debug, Clone, Default)]
pub struct EditRequest {
    pub subject: Option<String>,
    pub name: Option<String>,
    pub content: Option<String>,
}

impl EditRequest {
    /// True when no replacement values were given, i.e. the edit would change
    /// nothing.
    pub fn is_empty(&self) -> bool {
        self.subject.is_none() && self.name.is_none() && self.content.is_none()
    }
}

impl App {
    /// Apply the given replacements to the theorem with `id`, preserving its
    /// identity, `added_at`, and draw stats. Returns the updated theorem, or
    /// `None` if no theorem has that id.
    ///
    /// Resolving an id-prefix to a single id is a front-end concern (see
    /// [`App::find_by_id_prefix`]); this use-case acts on an exact id.
    pub fn edit(
        &mut self,
        id: &TheoremId,
        request: EditRequest,
    ) -> Result<Option<Theorem>, AppError> {
        let Some(mut theorem) = self.repo.get(id)? else {
            return Ok(None);
        };
        if let Some(subject) = request.subject {
            theorem.subject = Subject::new(subject)?;
        }
        if let Some(name) = request.name {
            theorem.name = Name::new(name)?;
        }
        if let Some(content) = request.content {
            theorem.content = LatexContent::new(content)?;
        }
        self.repo.update(theorem.clone())?;
        Ok(Some(theorem))
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

    fn add(app: &mut App) -> Theorem {
        app.add(AddRequest {
            subject: "Analysis".into(),
            name: "MCT".into(),
            content: "$\\lim$".into(),
        })
        .unwrap()
    }

    #[test]
    fn edit_replaces_only_the_given_fields() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let original = add(&mut app);

        let updated = app
            .edit(
                &original.id,
                EditRequest {
                    name: Some("Monotone Convergence".into()),
                    ..EditRequest::default()
                },
            )
            .unwrap()
            .unwrap();

        assert_eq!(updated.name.as_str(), "Monotone Convergence");
        assert_eq!(updated.subject, original.subject);
        assert_eq!(updated.content, original.content);
        assert_eq!(app.repo.get(&original.id).unwrap().unwrap(), updated);
    }

    #[test]
    fn edit_preserves_identity_timestamp_and_draw_stats() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let original = add(&mut app);

        let updated = app
            .edit(
                &original.id,
                EditRequest {
                    subject: Some("Real Analysis".into()),
                    name: Some("MCT (revised)".into()),
                    content: Some("$\\sup$".into()),
                },
            )
            .unwrap()
            .unwrap();

        assert_eq!(updated.id, original.id);
        assert_eq!(updated.added_at, original.added_at);
        assert_eq!(updated.draw_count, original.draw_count);
        assert_eq!(updated.last_drawn_at, original.last_drawn_at);
    }

    #[test]
    fn edit_unknown_id_reports_none() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        add(&mut app);

        let result = app
            .edit(
                &TheoremId::new(),
                EditRequest {
                    name: Some("anything".into()),
                    ..EditRequest::default()
                },
            )
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn edit_rejects_invalid_input_without_persisting() {
        let dir = tempdir().unwrap();
        let mut app = app_in(dir.path());
        let original = add(&mut app);

        let err = app
            .edit(
                &original.id,
                EditRequest {
                    subject: Some("   ".into()),
                    ..EditRequest::default()
                },
            )
            .unwrap_err();

        assert!(matches!(err, AppError::Domain(_)));
        assert_eq!(app.repo.get(&original.id).unwrap().unwrap(), original);
    }
}
