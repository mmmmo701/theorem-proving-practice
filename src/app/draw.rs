//! Use-case: draw a set of theorems and render them to a dated PDF — the daily
//! practice routine.

use std::path::PathBuf;

use chrono::{DateTime, Local, Utc};

use super::{App, AppError};
use crate::domain::Theorem;
use crate::render::OutputFormat;
use crate::selection::{DrawRequest, RandomSelector, Selector};

/// Options for a draw, all optional except the policy flags. `None` fields fall
/// back to configuration defaults.
#[derive(Debug, Clone)]
pub struct DrawOptions {
    /// How many theorems to draw; defaults to the configured count.
    pub count: Option<usize>,
    /// Seed for a reproducible draw.
    pub seed: Option<u64>,
    /// Directory to write the PDF into; defaults to the configured output dir.
    pub out_dir: Option<PathBuf>,
    /// Whether to replace an existing PDF for today.
    pub overwrite: bool,
    /// Render the PDF but do not record the draw (leave draw stats untouched).
    pub dry_run: bool,
    /// Use plain uniform random selection instead of the forgetting curve.
    pub uniform: bool,
    /// Output format to render (PDF by default, or HTML).
    pub format: OutputFormat,
}

/// The result of a successful draw: which theorems were chosen and where the
/// PDF was written.
#[derive(Debug, Clone)]
pub struct DrawOutcome {
    pub theorems: Vec<Theorem>,
    pub output_path: PathBuf,
}

impl App {
    /// Draw theorems from the library and compile them into today's PDF.
    ///
    /// Unless `dry_run` is set, a successful draw is recorded against each chosen
    /// theorem (bumping its draw count and last-drawn time) so the forgetting
    /// curve stays current. Recording happens only *after* a successful render,
    /// so a failed render never perturbs the stats.
    pub fn draw(&mut self, options: DrawOptions) -> Result<DrawOutcome, AppError> {
        let pool = self.repo.all()?;

        let count = options.count.unwrap_or(self.config.default_draw_count);
        // One reference time, shared by selection (recency) and recording (the
        // stamp), so the weight that chose a theorem and the time it gets agree.
        let now = Utc::now();
        let request = DrawRequest {
            count,
            seed: options.seed,
            now,
        };
        log::info!("drawing {} theorem(s) from a pool of {}", count, pool.len());
        let theorems = if options.uniform {
            RandomSelector::new().draw(&pool, &request)?
        } else {
            self.selector.draw(&pool, &request)?
        };

        let out_dir = options
            .out_dir
            .unwrap_or_else(|| self.config.output_dir.clone());
        let output_path = out_dir.join(format!(
            "practice-{}.{}",
            Local::now().format("%Y-%m-%d"),
            options.format.extension()
        ));
        log::debug!("rendering {:?} to {}", options.format, output_path.display());

        // Honor the no-clobber policy before doing the expensive render.
        if output_path.exists() && !options.overwrite {
            return Err(AppError::OutputExists { path: output_path });
        }

        // Dispatch to the renderer for the requested format. Trait-object method
        // calls don't need `Renderer` in scope.
        match options.format {
            OutputFormat::Pdf => self.renderer.render(&theorems, &output_path)?,
            OutputFormat::Html => self.html_renderer.render(&theorems, &output_path)?,
        };

        if !options.dry_run {
            self.record_draws(&theorems, now)?;
        }
        Ok(DrawOutcome {
            theorems,
            output_path,
        })
    }

    /// Persist a recorded draw for each theorem: bump its count and last-drawn
    /// time, then write it back through the repository.
    fn record_draws(&mut self, drawn: &[Theorem], at: DateTime<Utc>) -> Result<(), AppError> {
        for theorem in drawn {
            let mut updated = theorem.clone();
            updated.record_draw(at);
            self.repo.update(updated)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AddRequest;
    use crate::config::Config;
    use crate::render::{RenderError, RenderOutput, Renderer};
    use crate::storage::JsonStore;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::collections::HashSet;
    use tempfile::tempdir;

    /// A renderer that records how many times it ran and writes a stub file, so
    /// draw can be tested without a LaTeX engine.
    #[derive(Clone, Default)]
    struct FakeRenderer {
        calls: Arc<Mutex<usize>>,
    }
    impl Renderer for FakeRenderer {
        fn render(&self, theorems: &[Theorem], dest: &Path) -> Result<RenderOutput, RenderError> {
            *self.calls.lock().unwrap() += 1;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(dest, format!("stub pdf, {} theorems", theorems.len())).unwrap();
            Ok(RenderOutput {
                path: dest.to_path_buf(),
            })
        }
    }

    fn app_with(dir: &Path, renderer: FakeRenderer) -> App {
        let config = Config {
            data_dir: dir.join("data"),
            output_dir: dir.join("out"),
            ..Config::default()
        };
        let repo = JsonStore::new(config.store_path());
        App::new(config, Box::new(repo)).with_renderer(Box::new(renderer))
    }

    fn seed_pool(app: &mut App, n: usize) {
        for i in 0..n {
            app.add(AddRequest {
                subject: "Subject".into(),
                name: format!("Theorem {i}"),
                content: "$x$".into(),
            })
            .unwrap();
        }
    }

    fn options() -> DrawOptions {
        DrawOptions {
            count: None,
            seed: None,
            out_dir: None,
            overwrite: true,
            dry_run: false,
            uniform: false,
            format: OutputFormat::Pdf,
        }
    }

    #[test]
    fn draw_writes_pdf_and_returns_chosen_theorems() {
        let dir = tempdir().unwrap();
        let renderer = FakeRenderer::default();
        let mut app = app_with(dir.path(), renderer.clone());
        seed_pool(&mut app, 5);

        let outcome = app.draw(options()).unwrap();

        assert_eq!(outcome.theorems.len(), 3); // default count
        assert!(outcome.output_path.exists());
        assert_eq!(*renderer.calls.lock().unwrap(), 1);
    }

    #[test]
    fn html_format_uses_the_html_renderer_and_html_extension() {
        let dir = tempdir().unwrap();
        let pdf = FakeRenderer::default();
        let html = FakeRenderer::default();
        let mut app = app_with(dir.path(), pdf.clone())
            .with_html_renderer(Box::new(html.clone()));
        seed_pool(&mut app, 5);

        let outcome = app
            .draw(DrawOptions {
                format: OutputFormat::Html,
                ..options()
            })
            .unwrap();

        assert_eq!(outcome.output_path.extension().unwrap(), "html");
        // Only the HTML renderer ran; the PDF one was left untouched.
        assert_eq!(*html.calls.lock().unwrap(), 1);
        assert_eq!(*pdf.calls.lock().unwrap(), 0);
    }

    #[test]
    fn draw_respects_explicit_count() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default());
        seed_pool(&mut app, 5);

        let outcome = app
            .draw(DrawOptions {
                count: Some(2),
                ..options()
            })
            .unwrap();
        assert_eq!(outcome.theorems.len(), 2);
    }

    #[test]
    fn draw_with_too_few_theorems_is_a_selection_error() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default());
        seed_pool(&mut app, 1);

        let err = app.draw(options()).unwrap_err();
        assert!(matches!(err, AppError::Selection(_)));
    }

    #[test]
    fn no_clobber_refuses_to_overwrite_existing_pdf() {
        let dir = tempdir().unwrap();
        let renderer = FakeRenderer::default();
        let mut app = app_with(dir.path(), renderer.clone());
        seed_pool(&mut app, 5);

        // First draw writes today's PDF.
        app.draw(options()).unwrap();
        // Second, with overwrite disabled, must refuse without re-rendering.
        let err = app
            .draw(DrawOptions {
                overwrite: false,
                ..options()
            })
            .unwrap_err();

        assert!(matches!(err, AppError::OutputExists { .. }));
        assert_eq!(*renderer.calls.lock().unwrap(), 1);
    }

    #[test]
    fn seeded_draw_is_reproducible() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default());
        seed_pool(&mut app, 10);

        // dry_run keeps the library state fixed between the two draws, so the
        // seed alone determines the result (recording would evolve it).
        let opts = DrawOptions {
            seed: Some(7),
            dry_run: true,
            ..options()
        };
        let first: Vec<_> = app.draw(opts.clone()).unwrap().theorems.iter().map(|t| t.id).collect();
        let second: Vec<_> = app.draw(opts).unwrap().theorems.iter().map(|t| t.id).collect();
        assert_eq!(first, second);
    }

    /// A renderer that always fails, to prove a failed render records nothing.
    #[derive(Clone, Default)]
    struct FailingRenderer;
    impl Renderer for FailingRenderer {
        fn render(&self, _t: &[Theorem], _d: &Path) -> Result<RenderOutput, RenderError> {
            Err(RenderError::MissingOutput {
                expected: "nowhere.pdf".into(),
            })
        }
    }

    /// Sum of every stored theorem's draw count — a quick "was anything recorded".
    fn total_draws(app: &App) -> u32 {
        app.list().unwrap().iter().map(|t| t.draw_count).sum()
    }

    #[test]
    fn draw_records_each_chosen_theorem_once() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default());
        seed_pool(&mut app, 5);

        let outcome = app.draw(options()).unwrap();
        let chosen: HashSet<_> = outcome.theorems.iter().map(|t| t.id).collect();

        // Exactly the chosen theorems were stamped, each once.
        assert_eq!(total_draws(&app), 3);
        for t in app.list().unwrap() {
            let expected = if chosen.contains(&t.id) { 1 } else { 0 };
            assert_eq!(t.draw_count, expected);
            assert_eq!(t.last_drawn_at.is_some(), chosen.contains(&t.id));
        }
    }

    #[test]
    fn dry_run_records_nothing() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default());
        seed_pool(&mut app, 5);

        app.draw(DrawOptions {
            dry_run: true,
            ..options()
        })
        .unwrap();
        assert_eq!(total_draws(&app), 0);
    }

    #[test]
    fn a_failed_render_records_nothing() {
        let dir = tempdir().unwrap();
        let mut app = app_with(dir.path(), FakeRenderer::default())
            .with_renderer(Box::new(FailingRenderer));
        seed_pool(&mut app, 5);

        let err = app.draw(options()).unwrap_err();
        assert!(matches!(err, AppError::Render(_)));
        assert_eq!(total_draws(&app), 0);
    }
}
