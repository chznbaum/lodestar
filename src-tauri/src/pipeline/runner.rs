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

/// Build a completed `Step`, append it to the run note (durable projection), and return it.
/// `attempts` defaults to 1 here; the Task-6 claim-loop threads the real retry count.
pub(crate) fn record_step(
    vault_path: &str,
    run_id: &str,
    stage: &str,
    class: &str,
    target: &str,
    started_at: String,
    status: &str,
    error: Option<String>,
    cost: Option<i64>,
) -> Result<Step, String> {
    let step = Step {
        stage: stage.to_string(),
        class: class.to_string(),
        target: target.to_string(),
        status: status.to_string(),
        attempts: 1,
        started_at: Some(started_at),
        finished_at: Some(now_iso()),
        error,
        cost,
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
            record_step(vault_path, run_id, stage, "scrape", target, started, "ok", None, cost)
                .map_err(|e| ScrapeError {
                    status: None,
                    body: e,
                    class: crate::scraper::FailureClass::Transient,
                })?;
            Ok(result)
        }
        Err(e) => {
            record_step(
                vault_path, run_id, stage, "scrape", target, started, "failed",
                Some(e.to_string()), None,
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
            companies: vec!["stripe".into()],
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

        let scraper = FakeScraper { content: "<p>x</p>".into(), credits: 5 };
        let result = run_scrape_step(
            &vault, "2026-06-17-0001", "https://stripe.com/careers", "stripe",
            "careers-scrape", ProxyTier::Premium, &scraper,
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
            &vault, "2026-06-17-0001", "https://x", "stripe",
            "careers-scrape", ProxyTier::Premium, &FailScraper,
        )
        .unwrap_err();
        assert!(err.to_string().contains("403"));

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].status, "failed");
        assert!(reread.steps[0].error.as_deref().unwrap_or("").contains("403"));
        assert_eq!(reread.steps[0].cost, None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
