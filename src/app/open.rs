//! Use-case: enumerate the generated output files available to open.
//!
//! The app layer owns the output directory (via [`Config`]), so listing what
//! lives there belongs here. Actually launching a viewer is a terminal/OS
//! side-effect and stays in the CLI layer (`cli::input`), mirroring how the
//! `add` editor launch lives there rather than in the app.

use std::path::PathBuf;
use std::time::SystemTime;

use super::{App, AppError};

impl App {
    /// Generated output files (PDF/HTML/…) in the configured output directory,
    /// most-recently-modified first so today's draw sorts to the top.
    ///
    /// Sub-directories and dotfiles are skipped. A never-used output directory
    /// that does not exist yet is treated as empty rather than an error.
    pub fn list_outputs(&self) -> Result<Vec<PathBuf>, AppError> {
        let dir = &self.config.output_dir;
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(AppError::OutputListing {
                    path: dir.clone(),
                    source,
                });
            }
        };

        let mut entries: Vec<(SystemTime, PathBuf)> = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|source| AppError::OutputListing {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();

            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            let hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if !is_file || hidden {
                continue;
            }

            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((modified, path));
        }

        // Newest first; tie-break on path so equal timestamps stay deterministic.
        entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        Ok(entries.into_iter().map(|(_, path)| path).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::storage::JsonStore;
    use tempfile::tempdir;

    fn app_with_output(dir: &std::path::Path) -> App {
        let config = Config {
            data_dir: dir.join("data"),
            output_dir: dir.join("output"),
            ..Config::default()
        };
        let repo = JsonStore::new(config.store_path());
        App::new(config, Box::new(repo))
    }

    #[test]
    fn missing_output_dir_lists_as_empty() {
        let dir = tempdir().unwrap();
        let app = app_with_output(dir.path());
        assert!(app.list_outputs().unwrap().is_empty());
    }

    #[test]
    fn lists_files_but_skips_subdirs_and_dotfiles() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("output");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("today.pdf"), b"pdf").unwrap();
        std::fs::write(out.join("today.html"), b"html").unwrap();
        std::fs::write(out.join(".hidden"), b"x").unwrap();
        std::fs::create_dir(out.join("build")).unwrap();

        let app = app_with_output(dir.path());
        let names: Vec<String> = app
            .list_outputs()
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"today.pdf".to_string()));
        assert!(names.contains(&"today.html".to_string()));
    }
}
