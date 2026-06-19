//! The discovery chain as **step-level queue tasks** (the design's "queue tasks ARE the
//! steps" model). Each task is one retryable unit; on success it enqueues its successor
//! (carrying its output in the payload), so a retry never re-does upstream successful work —
//! a `structure-listings` retry re-uses the saved scraped text (no re-scrape), a `finalize`
//! retry re-uses the saved listings (no re-LLM). This generalizes to Phase B's per-role
//! JD fetches (N independent tasks; only the failures retry).
//!
//! Discovery = 3 tasks per company:
//!   `careers-scrape` (scrape+sanitize) → `structure-listings` (LLM) → `finalize` (filter+write).
//!
//! `dispatch` executes one step + projects telemetry into the `checks/` note; `pump_once`
//! drives the queue (claim → dispatch → enqueue successors / retry). The Task-6 worker thread
//! loops `pump_once` off the tokio reactor with a real Tauri `EventSink`; tests drive it with
//! the fakes + `NoopSink` (zero spend).
#![allow(dead_code)]

use crate::check::{get_check, write_check, Check};
use crate::config::{model_for, PipelineConfig};
use crate::job::{job_slug, list_jobs, write_job_stub, Job};
use crate::llm::Llm;
use crate::pipeline::filter::{prefilter, RawListing};
use crate::pipeline::queue::{NewTask, Queue, QueuedTask, MAX_ATTEMPTS};
use crate::pipeline::runner::{now_iso, record_step, run_scrape_step};
use crate::profile::read_target_criteria;
use crate::prompts::{build_structure_listings_prompt, parse_structured_listings, StructuredListing};
use crate::sanitize::sanitize;
use crate::scraper::Scraper;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Live progress hook. The Task-6 worker passes a Tauri-emitting impl; tests pass `NoopSink`.
pub trait EventSink: Send + Sync {
    fn step_done(&self, run_id: &str, stage: &str, status: &str);
    fn run_finished(&self, run_id: &str, status: &str);
}

pub struct NoopSink;
impl EventSink for NoopSink {
    fn step_done(&self, _run_id: &str, _stage: &str, _status: &str) {}
    fn run_finished(&self, _run_id: &str, _status: &str) {}
}

// Inter-step payloads, carried (durably) in the queue task so a retry re-uses prior output.
#[derive(Serialize, Deserialize)]
struct ScrapePayload {
    careers_url: String,
}
#[derive(Serialize, Deserialize)]
struct StructurePayload {
    sanitized: String,
}
#[derive(Serialize, Deserialize)]
struct FinalizePayload {
    listings: Vec<StructuredListing>,
}

/// `<today>-NNNN`, sequenced per day over existing `checks/` notes.
pub fn next_run_id(vault_path: &str, today: &str) -> Result<String, String> {
    let dir = Path::new(vault_path).join("checks");
    let count = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with(today) && n.ends_with(".md"))
                    .unwrap_or(false)
            })
            .count(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(format!("read checks dir: {e}")),
    };
    Ok(format!("{today}-{:04}", count + 1))
}

/// Open a `checks/` run and enqueue the first discovery task. Returns the run id.
pub fn start_discovery(
    queue: &dyn Queue,
    vault_path: &str,
    company_slug: &str,
    careers_url: &str,
    today: &str,
) -> Result<String, String> {
    let base = Path::new(vault_path);
    std::fs::create_dir_all(base.join("checks")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(base.join("jobs")).map_err(|e| e.to_string())?;

    let run_id = next_run_id(vault_path, today)?;
    let run = Check {
        slug: run_id.clone(),
        kind: "job_check".into(),
        trigger: "manual".into(),
        status: "running".into(),
        started_at: Some(now_iso()),
        finished_at: None,
        duration: None,
        companies: vec![company_slug.to_string()],
        roles_found: 0,
        jds_fetched: 0,
        errors: 0,
        steps: vec![],
    };
    write_check(vault_path, &run)?;

    let payload = serde_json::to_string(&ScrapePayload { careers_url: careers_url.to_string() })
        .map_err(|e| e.to_string())?;
    queue.enqueue(NewTask {
        run_id: run_id.clone(),
        stage: "careers-scrape".into(),
        class: "scrape".into(),
        target: company_slug.to_string(),
        payload,
    })?;
    Ok(run_id)
}

/// Claim and execute at most one queued task. Returns `Ok(true)` if a task was processed,
/// `Ok(false)` if the queue is empty. On step success, the successor task is enqueued and the
/// task completed; on failure the queue retries it (or, on the terminal attempt, the run is
/// marked `failed`).
pub fn pump_once<S: Scraper, L: Llm>(
    queue: &dyn Queue,
    vault_path: &str,
    cfg: &PipelineConfig,
    scraper: &S,
    llm: &L,
    sink: &dyn EventSink,
    is_cancelled: &dyn Fn(&str) -> bool,
) -> Result<bool, String> {
    let Some(task) = queue.claim_next()? else {
        return Ok(false);
    };
    if is_cancelled(&task.run_id) {
        // the run was cancelled: drop this task and the rest of its chain without dispatching
        queue.complete(task.id)?;
        sink.step_done(&task.run_id, &task.stage, "cancelled");
        return Ok(true);
    }
    match dispatch(&task, vault_path, cfg, scraper, llm) {
        Ok(successors) => {
            for s in successors {
                queue.enqueue(s)?;
            }
            queue.complete(task.id)?;
            sink.step_done(&task.run_id, &task.stage, "ok");
            if task.stage == "finalize" {
                sink.run_finished(&task.run_id, "awaiting_input");
            }
        }
        Err(e) => {
            queue.fail(task.id, &e)?;
            sink.step_done(&task.run_id, &task.stage, "failed");
            // Terminal failure (retries exhausted): the chain can't proceed — mark the run failed
            // so it doesn't sit "running" forever.
            if task.attempts >= MAX_ATTEMPTS {
                if let Ok(mut run) = get_check(vault_path.to_string(), task.run_id.clone()) {
                    run.status = "failed".into();
                    run.errors += 1;
                    run.finished_at = Some(now_iso());
                    let _ = write_check(vault_path, &run);
                    sink.run_finished(&task.run_id, "failed");
                }
            }
        }
    }
    Ok(true)
}

/// Execute one pipeline step. Returns the successor task(s) to enqueue (empty when the chain
/// ends). Records the step's telemetry into the `checks/` note as a side effect.
fn dispatch<S: Scraper, L: Llm>(
    task: &QueuedTask,
    vault_path: &str,
    cfg: &PipelineConfig,
    scraper: &S,
    llm: &L,
) -> Result<Vec<NewTask>, String> {
    let run_id = task.run_id.as_str();
    let company = task.target.as_str();
    match task.stage.as_str() {
        "careers-scrape" => {
            let p: ScrapePayload = serde_json::from_str(&task.payload).map_err(|e| e.to_string())?;
            let scraped = run_scrape_step(vault_path, run_id, &p.careers_url, company, scraper)?;
            let started = now_iso();
            let sanitized = sanitize(&scraped.content, &p.careers_url);
            record_step(vault_path, run_id, "sanitize", "script", company, started, "ok", None, None)?;
            Ok(vec![NewTask {
                run_id: run_id.to_string(),
                stage: "structure-listings".into(),
                class: "llm".into(),
                target: company.to_string(),
                payload: serde_json::to_string(&StructurePayload { sanitized })
                    .map_err(|e| e.to_string())?,
            }])
        }
        "structure-listings" => {
            let p: StructurePayload =
                serde_json::from_str(&task.payload).map_err(|e| e.to_string())?;
            let started = now_iso();
            let prompt =
                build_structure_listings_prompt(&model_for(cfg, "structure-listings"), &p.sanitized);
            let listings: Vec<StructuredListing> = match llm.complete(&prompt) {
                Ok(resp) => {
                    let cost = resp.cost_micro_usd;
                    match parse_structured_listings(&resp.content) {
                        Ok(l) => {
                            record_step(vault_path, run_id, "structure-listings", "llm", company, started, "ok", None, cost)?;
                            l
                        }
                        Err(e) => {
                            record_step(vault_path, run_id, "structure-listings", "llm", company, started, "failed", Some(e.clone()), cost)?;
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    record_step(vault_path, run_id, "structure-listings", "llm", company, started, "failed", Some(e.clone()), None)?;
                    return Err(e);
                }
            };
            Ok(vec![NewTask {
                run_id: run_id.to_string(),
                stage: "finalize".into(),
                class: "script".into(),
                target: company.to_string(),
                payload: serde_json::to_string(&FinalizePayload { listings })
                    .map_err(|e| e.to_string())?,
            }])
        }
        "finalize" => {
            let p: FinalizePayload =
                serde_json::from_str(&task.payload).map_err(|e| e.to_string())?;
            let started = now_iso();
            let today = run_id.get(..10).unwrap_or(run_id); // run id is date-prefixed
            let criteria = read_target_criteria(vault_path)?;
            let existing: HashSet<String> =
                list_jobs(vault_path.to_string())?.into_iter().filter_map(|j| j.url).collect();
            let with_url: Vec<StructuredListing> =
                p.listings.into_iter().filter(|l| l.url.is_some()).collect();
            let raws: Vec<RawListing> = with_url
                .iter()
                .map(|l| RawListing {
                    title: l.title.clone(),
                    url: l.url.clone().unwrap_or_default(),
                    location: l.location.clone(),
                })
                .collect();
            let kept_urls: HashSet<String> =
                prefilter(raws, &criteria, &existing).into_iter().map(|r| r.url).collect();
            let selected: Vec<&StructuredListing> = with_url
                .iter()
                .filter(|l| l.url.as_deref().map(|u| kept_urls.contains(u)).unwrap_or(false))
                .collect();
            record_step(vault_path, run_id, "pre-filter", "script", company, started, "ok", None, None)?;

            for listing in &selected {
                let job = Job {
                    slug: job_slug(&listing.title, company),
                    title: listing.title.clone(),
                    company: Some(company.to_string()),
                    url: listing.url.clone(),
                    classification: listing.classification.clone(),
                    location: listing.location.clone(),
                    comp_low: None,
                    comp_high: None,
                    comp_currency: None,
                    comp_raw: None,
                    date_posted: None,
                    last_seen: Some(today.to_string()),
                    ats: listing.ats.clone(),
                    tech_stack: vec![],
                    fit_score: None,
                    status: Some("new".to_string()),
                    skip_reason: None,
                    jd_raw_file: None,
                    jd_fetched: false,
                };
                if let Err(e) = write_job_stub(vault_path, &job) {
                    eprintln!("skip stub {}: {e}", job.slug);
                }
            }

            let mut run = get_check(vault_path.to_string(), run_id.to_string())?;
            run.roles_found = selected.len() as u32;
            run.status = "awaiting_input".into();
            run.finished_at = Some(now_iso());
            write_check(vault_path, &run)?;
            Ok(vec![])
        }
        other => Err(format!("unknown stage: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::llm::{Llm, LlmRequest, LlmResponse};
    use crate::pipeline::queue::SqliteQueue;
    use crate::scraper::{ScrapeResult, Scraper};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU32, Ordering};

    static SEQ: AtomicU32 = AtomicU32::new(0);

    fn setup_vault() -> (std::path::PathBuf, String) {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lodestar-disc-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        std::fs::create_dir_all(dir.join("profile")).unwrap();
        std::fs::write(
            dir.join("profile/target_criteria.md"),
            "---\ntype: target_criteria\nlocation_requirement: remote_only\nmatch_titles:\n  - engineer\n---\n",
        )
        .unwrap();
        let vault = dir.to_str().unwrap().to_string();
        (dir, vault)
    }

    fn two_listings() -> String {
        r#"[{"title":"Senior Engineer","url":"https://co/1","location":"Remote","classification":"senior-ic"},
            {"title":"Real Estate Agent","url":"https://co/2","location":"Remote"}]"#
            .into()
    }

    fn drain<S: Scraper, L: Llm>(q: &SqliteQueue, vault: &str, s: &S, l: &L) {
        while pump_once(q, vault, &default_config(), s, l, &NoopSink, &|_| false).unwrap() {}
    }

    // Counting fakes (interior mutability; the test is single-threaded).
    struct CountingScraper {
        content: String,
        credits: u32,
        calls: Cell<u32>,
    }
    impl Scraper for CountingScraper {
        fn fetch(&self, _url: &str) -> Result<ScrapeResult, String> {
            self.calls.set(self.calls.get() + 1);
            Ok(ScrapeResult { content: self.content.clone(), credits: Some(self.credits) })
        }
    }
    struct FlakyLlm {
        reply: String,
        calls: Cell<u32>,
    }
    impl Llm for FlakyLlm {
        fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse, String> {
            let n = self.calls.get() + 1;
            self.calls.set(n);
            if n == 1 {
                Err("rate limited".into()) // fail the first attempt
            } else {
                Ok(LlmResponse { content: self.reply.clone(), cost_micro_usd: Some(20_000) }) // $0.02
            }
        }
    }

    #[test]
    fn discovery_drains_to_filtered_stubs_and_awaiting_input() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = FlakyLlm { reply: two_listings(), calls: Cell::new(1) }; // start at 1 -> always succeed

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "awaiting_input");
        assert_eq!(run.roles_found, 1); // agent filtered out
        assert!(run.steps.iter().any(|s| s.stage == "careers-scrape" && s.cost == Some(5)));
        assert!(run.steps.iter().any(|s| s.stage == "structure-listings" && s.cost == Some(20_000)));
        assert!(dir.join("jobs/senior-engineer-acme.md").exists());
        assert!(!dir.join("jobs/real-estate-agent-acme.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn structure_failure_retries_without_rescraping() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = FlakyLlm { reply: two_listings(), calls: Cell::new(0) }; // fails once, then succeeds

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // The structure step failed once and retried — but the scrape ran exactly once (no re-scrape).
        assert_eq!(scraper.calls.get(), 1, "scrape must NOT be redone when structure retries");
        assert_eq!(llm.calls.get(), 2, "structure retried after its first failure");
        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "awaiting_input"); // recovered to completion
        assert_eq!(run.roles_found, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
