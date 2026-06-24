//! The step executor: run one pipeline stage via the injected `Scraper`/`Llm`, project a
//! telemetry `Step` into the open `checks/` run note, and return the stage's output for the
//! next stage. Pure-enough to unit-test with the test-suite fakes (zero spend).
//!
//! Event emission + the queue claim-loop (which need the Tauri `AppHandle`) live in the app
//! integration (Task 6); keeping them out here is what lets this be tested without a live app.
//! Like the scrapers/LLM clients, the loop must run off the tokio reactor (`reqwest::blocking`).
// Consumed by the discovery chain (Task 5) + app (Task 6); suppress dead-code until wired.
#![allow(dead_code)]

use crate::check::{append_step, Step};
use crate::scraper::{ProxyTier, ScrapeError, ScrapeResult, Scraper};
use chrono::Local;

/// Wall-clock timestamp for step start/finish, matching the `checks/` note convention.
pub(crate) fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// What step this is and when it started — the durable *identity* half of a recorded `Step`,
/// bundled so the recording helpers stay well under clippy's argument threshold. `vault_path` and
/// `run_id` (the *address* of the run note to append to) stay as separate args: they're the
/// destination, not part of the step's identity. `finished_at`/`attempts` aren't here — the helper
/// stamps `finished_at = now_iso()` and `attempts = 1` (the Task-6 claim-loop threads the real
/// retry count) at record time.
pub(crate) struct StepIdentity<'a> {
    pub stage: &'a str,
    pub class: &'a str,
    pub target: &'a str,
    pub started_at: String,
}

impl<'a> StepIdentity<'a> {
    /// Convenience constructor so call sites read `StepIdentity::new(stage, class, target, started)`.
    pub fn new(stage: &'a str, class: &'a str, target: &'a str, started_at: String) -> Self {
        Self {
            stage,
            class,
            target,
            started_at,
        }
    }
}

/// The *outcome* half of a recorded `Step`: terminal status plus telemetry (`error`, `warnings`,
/// `cost`, cache tokens). Build it with the per-variant constructors, which encode the defaults the
/// old `record_*` family encoded positionally:
/// - [`StepOutcome::ok`] — status `"ok"`, no error/warnings.
/// - [`StepOutcome::failed`] — status `"failed"`, carrying the error.
/// - [`StepOutcome::warned`] — status `"warning"`, error `None`, asserts the warnings are non-empty.
///
/// Cache tokens default to `None` (scrape/script steps and llm steps that don't cache honestly
/// record `None`); the llm arm layers them on with [`StepOutcome::with_cache`]. This replaces the
/// old `record_step`/`record_llm_step` split — there's one recording path now, and the caller says
/// whether cache tokens exist rather than which function to reach for.
pub(crate) struct StepOutcome {
    pub status: &'static str,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub cost: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
}

impl StepOutcome {
    /// A successful step: status `"ok"`, no error, no warnings; `cost` as charged (or `None`).
    pub fn ok(cost: Option<i64>) -> Self {
        Self {
            status: "ok",
            error: None,
            warnings: vec![],
            cost,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    /// A failed step: status `"failed"`, carrying the error message; `cost` as charged (or `None`).
    pub fn failed(error: String, cost: Option<i64>) -> Self {
        Self {
            status: "failed",
            error: Some(error),
            warnings: vec![],
            cost,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    /// A warned step: status `"warning"`, error `None`, recording the (non-empty) warnings. Asserts
    /// at least one warning, matching the old `record_step_warned`/`record_llm_step_warned` guard.
    pub fn warned(warnings: Vec<String>, cost: Option<i64>) -> Self {
        debug_assert!(
            !warnings.is_empty(),
            "StepOutcome::warned requires at least one warning"
        );
        Self {
            status: "warning",
            error: None,
            warnings,
            cost,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    /// Attach the cache read/write token counts from an `LlmResponse` (the old `record_llm_step*`
    /// path). Observability only — proves the prompt cache is engaging; `cost` already nets the
    /// discount. Used only by llm steps that cache (alignment today); everything else leaves the
    /// defaults (`None`).
    pub fn with_cache(
        mut self,
        cache_read_tokens: Option<i64>,
        cache_write_tokens: Option<i64>,
    ) -> Self {
        self.cache_read_tokens = cache_read_tokens;
        self.cache_write_tokens = cache_write_tokens;
        self
    }
}

/// Build a completed `Step` from its identity + outcome, append it to the run note (durable
/// projection), and return it. `attempts` defaults to 1; the Task-6 claim-loop threads the real
/// retry count. `finished_at` is stamped here.
pub(crate) fn record_step(
    vault_path: &str,
    run_id: &str,
    identity: StepIdentity<'_>,
    outcome: StepOutcome,
) -> Result<Step, String> {
    let step = Step {
        stage: identity.stage.to_string(),
        class: identity.class.to_string(),
        target: identity.target.to_string(),
        status: outcome.status.to_string(),
        attempts: 1,
        started_at: Some(identity.started_at),
        finished_at: Some(now_iso()),
        error: outcome.error,
        cost: outcome.cost,
        cache_read_tokens: outcome.cache_read_tokens,
        cache_write_tokens: outcome.cache_write_tokens,
        warnings: outcome.warnings,
    };
    append_step(vault_path, run_id, step.clone())?;
    Ok(step)
}

/// Execute a scrape stage: scrape `url`, record an `ok`/`failed` step (cost = the
/// ScrapingBee credits charged), and return the scraped content for sanitization. On
/// scraper failure, records a `failed` step and propagates the `ScrapeError`.
/// `stage` is the queue stage name (e.g. `"careers-scrape"` or `"jd-scrape"`) so that
/// the recorded step row carries the correct stage label.
pub fn run_scrape_step<S: Scraper>(
    vault_path: &str,
    run_id: &str,
    url: &str,
    target: &str,
    stage: &str,
    tier: ProxyTier,
    scraper: &S,
) -> Result<ScrapeResult, ScrapeError> {
    let started = now_iso();
    match scraper.fetch(url, tier) {
        Ok(result) => {
            let cost = result.credits.map(|c| c as i64);
            record_step(
                vault_path,
                run_id,
                StepIdentity::new(stage, "scrape", target, started),
                StepOutcome::ok(cost),
            )
            .map_err(|e| ScrapeError {
                status: None,
                body: e,
                class: crate::scraper::FailureClass::Transient,
            })?;
            Ok(result)
        }
        Err(e) => {
            record_step(
                vault_path,
                run_id,
                StepIdentity::new(stage, "scrape", target, started),
                StepOutcome::failed(e.to_string(), None),
            )
            .map_err(|re| ScrapeError {
                status: None,
                body: re,
                class: crate::scraper::FailureClass::Transient,
            })?;
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{get_check, write_check, Check};
    use crate::scraper::tests::FakeScraper;
    use crate::scraper::{FailureClass, ScrapeError};

    fn open_run(vault: &str) {
        let run = Check {
            slug: "2026-06-17-0001".into(),
            kind: "job_check".into(),
            trigger: "manual".into(),
            status: "running".into(),
            started_at: Some("2026-06-17T10:00:00".into()),
            finished_at: None,
            duration: None,
            subject: "stripe".into(),
            roles_found: 0,
            errors: 0,
            steps: vec![],
        };
        write_check(vault, &run).unwrap();
    }

    #[test]
    fn scrape_step_records_ok_step_with_credits_and_returns_content() {
        let dir = std::env::temp_dir().join(format!("lodestar-runner-ok-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        let scraper = FakeScraper {
            content: "<p>x</p>".into(),
            credits: 5,
        };
        let result = run_scrape_step(
            &vault,
            "2026-06-17-0001",
            "https://stripe.com/careers",
            "stripe",
            "careers-scrape",
            ProxyTier::Premium,
            &scraper,
        )
        .unwrap();
        assert_eq!(result.content, "<p>x</p>"); // content flows to the next stage

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].stage, "careers-scrape");
        assert_eq!(reread.steps[0].status, "ok");
        assert_eq!(reread.steps[0].cost, Some(5)); // credits Some(5) -> cost Some(5)
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scrape_step_records_failed_step_and_propagates_error() {
        let dir = std::env::temp_dir().join(format!("lodestar-runner-fail-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        struct FailScraper;
        impl Scraper for FailScraper {
            fn fetch(&self, _url: &str, _tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
                Err(ScrapeError {
                    status: Some(403),
                    body: "403 blocked".into(),
                    class: FailureClass::EscalateProxy,
                })
            }
        }
        let err = run_scrape_step(
            &vault,
            "2026-06-17-0001",
            "https://x",
            "stripe",
            "careers-scrape",
            ProxyTier::Premium,
            &FailScraper,
        )
        .unwrap_err();
        assert!(err.to_string().contains("403"));

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].status, "failed");
        assert!(reread.steps[0]
            .error
            .as_deref()
            .unwrap_or("")
            .contains("403"));
        assert_eq!(reread.steps[0].cost, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Warning recorder tests (TDD RED → GREEN) ─────────────────────────

    #[test]
    fn record_step_warned_writes_warning_status_and_warnings() {
        // record_step_warned must write a step whose status is "warning",
        // whose warnings match, and whose error is None.
        let dir = std::env::temp_dir().join(format!("lodestar-runner-warn-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        let started = "2026-06-21T10:00:00".to_string();
        let warnings = vec![
            "rejected countries: expected array, got string".to_string(),
            "missing equity field".to_string(),
        ];
        let step = record_step(
            &vault,
            "2026-06-17-0001",
            StepIdentity::new("research-gaps", "llm", "stripe", started),
            StepOutcome::warned(warnings.clone(), Some(1_000)),
        )
        .unwrap();

        assert_eq!(step.status, "warning");
        assert_eq!(step.warnings, warnings);
        assert!(step.error.is_none());

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].status, "warning");
        assert_eq!(reread.steps[0].warnings, warnings);
        assert!(reread.steps[0].error.is_none());
        assert_eq!(reread.steps[0].cost, Some(1_000));
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Cache-token recording tests (Task 2a, TDD RED → GREEN) ───────────

    #[test]
    fn record_llm_step_persists_cache_tokens() {
        // The llm recording path must persist cache read/write counts onto the appended Step,
        // readable back through parse_check.
        let dir =
            std::env::temp_dir().join(format!("lodestar-runner-llmcache-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        record_step(
            &vault,
            "2026-06-17-0001",
            StepIdentity::new(
                "alignment",
                "llm",
                "stripe",
                "2026-06-23T10:00:00".to_string(),
            ),
            StepOutcome::ok(Some(4_200)).with_cache(Some(6_600), Some(7_000)),
        )
        .unwrap();

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].stage, "alignment");
        assert_eq!(reread.steps[0].cache_read_tokens, Some(6_600));
        assert_eq!(reread.steps[0].cache_write_tokens, Some(7_000));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_llm_step_warned_persists_cache_tokens() {
        // The warned llm path must carry both the warnings and the cache tokens.
        let dir = std::env::temp_dir().join(format!(
            "lodestar-runner-llmwarncache-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        record_step(
            &vault,
            "2026-06-17-0001",
            StepIdentity::new(
                "alignment",
                "llm",
                "stripe",
                "2026-06-23T10:00:00".to_string(),
            ),
            StepOutcome::warned(vec!["narrative truncated".into()], Some(4_200))
                .with_cache(Some(6_600), None),
        )
        .unwrap();

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].status, "warning");
        assert_eq!(
            reread.steps[0].warnings,
            vec!["narrative truncated".to_string()]
        );
        assert_eq!(reread.steps[0].cache_read_tokens, Some(6_600));
        assert_eq!(reread.steps[0].cache_write_tokens, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_step_records_none_cache_tokens() {
        // The scrape/script path (unchanged signature) must record None for both cache fields.
        let dir =
            std::env::temp_dir().join(format!("lodestar-runner-nocache-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        record_step(
            &vault,
            "2026-06-17-0001",
            StepIdentity::new(
                "careers-scrape",
                "scrape",
                "stripe",
                "2026-06-23T10:00:00".to_string(),
            ),
            StepOutcome::ok(Some(5)),
        )
        .unwrap();

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert!(reread.steps[0].cache_read_tokens.is_none());
        assert!(reread.steps[0].cache_write_tokens.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_step_ok_emits_no_warnings_key_in_note() {
        // An ok step written by record_step must not produce a "warnings:" key in the note.
        let dir = std::env::temp_dir().join(format!("lodestar-runner-ok2-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        open_run(&vault);

        record_step(
            &vault,
            "2026-06-17-0001",
            StepIdentity::new(
                "careers-scrape",
                "scrape",
                "stripe",
                "2026-06-21T10:00:00".to_string(),
            ),
            StepOutcome::ok(Some(5)),
        )
        .unwrap();

        let path = dir.join("checks").join("2026-06-17-0001.md");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("warnings:"),
            "ok step must not emit warnings key; got:\n{text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
