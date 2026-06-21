//! The `job_check` discovery chain as **step-level queue tasks** (the design's "queue tasks
//! ARE the steps" model). Each task is one retryable unit; on success it enqueues its
//! successor (carrying its output in the payload), so a retry never re-does upstream
//! successful work — a `structure-listings` retry re-uses the saved scraped text (no
//! re-scrape), a `finalize` retry re-uses the saved listings (no re-LLM).
//!
//! `job_check` is a standalone listing-discovery run. It ends in `complete` once listings
//! are filtered and written as job stubs. Per-job detail (`job_detail`) is a separate future
//! check kind — its own run, not a step on this chain.
//!
//! Discovery = 3 tasks per company:
//!   `careers-scrape` (scrape+sanitize) → `structure-listings` (LLM) → `finalize` (filter+write).
//!
//! `dispatch` executes one step + projects telemetry into the `checks/` note; `pump_once`
//! drives the queue (claim → dispatch → enqueue successors / retry). The Task-6 worker thread
//! loops `pump_once` off the tokio reactor with a real Tauri `EventSink`; tests drive it with
//! the fakes + `NoopSink` (zero spend).
//!
//! ## Scrape failure policy (per `FailureClass`)
//!
//! - `Terminal`      → mark task dead immediately; mark run `failed` — no retry, no spend.
//! - `FixEncoding`   → re-issue once with RFC-3986-percent-encoded URL; if still failing, Terminal.
//! - `EscalateProxy` → re-enqueue once with `ProxyTier::Stealth` (75cr); if still failing, Terminal.
//!   Never escalate a `Terminal` (404); escalation only fires once (Stealth doesn't escalate again).
//! - `Transient`     → bounded backoff retry via the queue's `fail()` mechanism, capped at
//!   `TRANSIENT_SCRAPE_MAX_ATTEMPTS` (≤2). Non-scrape stages (LLM) keep `MAX_ATTEMPTS`.
#![allow(dead_code)]

use crate::check::{get_check, write_check, Check};
use crate::config::{model_for, PipelineConfig};
use crate::job::{
    job_slug, list_jobs, parse_job, set_job_list_field, set_job_section, update_job_field,
    write_job_stub, Job, EMPLOYMENT_TYPES, REMOTE_KINDS, SPONSORSHIP, VALID_LEVELS,
};
use crate::llm::Llm;
use crate::pipeline::filter::{prefilter, RawListing};
use crate::pipeline::queue::{NewTask, Queue, QueuedTask, MAX_ATTEMPTS, TRANSIENT_SCRAPE_MAX_ATTEMPTS};
use crate::pipeline::runner::{now_iso, record_step, run_scrape_step};
use crate::profile::read_target_criteria;
use crate::prompts::{
    build_structure_jd_prompt, build_structure_listings_prompt, parse_structured_jd,
    parse_structured_listings, StructuredJd, StructuredListing,
};
use crate::sanitize::sanitize;
use crate::scraper::{percent_encode_target_url, FailureClass, ProxyTier, Scraper};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Live progress hook. The Task-6 worker passes a Tauri-emitting impl; tests pass `NoopSink`.
pub trait EventSink: Send + Sync {
    fn step_done(&self, run_id: &str, stage: &str, status: &str);
    fn run_finished(&self, run_id: &str, status: &str);
    /// Emitted immediately after a task is claimed, before execution. Used by the UI to
    /// display the current stage label ("Scraping careers page…" etc.) in real time.
    /// `detail` carries optional sub-phase info (e.g. `Some("stealth")` for the stealth-proxy
    /// re-enqueue attempt) so the UI can show a more specific label.
    fn step_started(&self, run_id: &str, stage: &str, detail: Option<&str>);
}

pub struct NoopSink;
impl EventSink for NoopSink {
    fn step_done(&self, _run_id: &str, _stage: &str, _status: &str) {}
    fn run_finished(&self, _run_id: &str, _status: &str) {}
    fn step_started(&self, _run_id: &str, _stage: &str, _detail: Option<&str>) {}
}

/// Write `last_checked = <today>` on the target company note. Ignores errors (logs them) so a
/// failed write never aborts the run. Today is derived from the first 10 chars of `run_id`
/// (the date-prefix `YYYY-MM-DD`).
fn stamp_checked(vault_path: &str, company_slug: &str, today: &str) {
    if let Err(e) = crate::company::update_company_field(
        vault_path.to_string(),
        company_slug.to_string(),
        "last_checked".to_string(),
        today.to_string(),
    ) {
        eprintln!("stamp_checked({company_slug}): {e}");
    }
}

// Inter-step payloads, carried (durably) in the queue task so a retry re-uses prior output.
#[derive(Serialize, Deserialize)]
struct ScrapePayload {
    careers_url: String,
    /// Which proxy tier to use. Defaults to `"premium"` on the first attempt;
    /// set to `"stealth"` when the task is re-enqueued after an `EscalateProxy` failure.
    #[serde(default = "default_tier")]
    tier: String,
    /// `true` when this task was re-enqueued after a `FixEncoding` failure.
    /// If encoding is already fixed and we fail again, treat the failure as Terminal.
    #[serde(default)]
    encoding_fixed: bool,
}

fn default_tier() -> String {
    "premium".into()
}

impl ScrapePayload {
    fn proxy_tier(&self) -> ProxyTier {
        if self.tier == "stealth" { ProxyTier::Stealth } else { ProxyTier::Premium }
    }
}

#[derive(Serialize, Deserialize)]
struct StructurePayload {
    sanitized: String,
}
#[derive(Serialize, Deserialize)]
struct FinalizePayload {
    listings: Vec<StructuredListing>,
}

/// Carries the job slug + sanitized JD text through the `jd-scrape → structure-jd` stages.
/// `slug` is the durable carrier (mirrors `task.target`, but the payload survives re-enqueue).
#[derive(Serialize, Deserialize)]
struct JdStructurePayload {
    slug: String,
    sanitized: String,
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
        errors: 0,
        steps: vec![],
    };
    write_check(vault_path, &run)?;

    let payload = serde_json::to_string(&ScrapePayload {
        careers_url: careers_url.to_string(),
        tier: default_tier(),
        encoding_fixed: false,
    })
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

/// Open a `checks/` run for the `job_detail` chain and enqueue the first `jd-scrape` task.
/// Reads `url` and `company` from the job note at `<vault>/jobs/<job_slug>.md`.
/// Returns the run id.
pub fn start_job_detail(
    queue: &dyn Queue,
    vault_path: &str,
    job_slug: &str,
    today: &str,
) -> Result<String, String> {
    let path = Path::new(vault_path).join("jobs").join(format!("{job_slug}.md"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    let job = parse_job(job_slug, &text)?;
    let url = job.url.ok_or_else(|| format!("job {job_slug:?} has no url to scrape"))?;
    let company = job.company.unwrap_or_default();
    std::fs::create_dir_all(Path::new(vault_path).join("checks")).map_err(|e| e.to_string())?;
    let run_id = next_run_id(vault_path, today)?;
    let run = Check {
        slug: run_id.clone(),
        kind: "job_detail".into(),
        trigger: "manual".into(),
        status: "running".into(),
        started_at: Some(now_iso()),
        finished_at: None,
        duration: None,
        companies: if company.is_empty() { vec![] } else { vec![company] },
        roles_found: 0,
        errors: 0,
        steps: vec![],
    };
    write_check(vault_path, &run)?;
    let payload = serde_json::to_string(&ScrapePayload {
        careers_url: url,
        tier: default_tier(),
        encoding_fixed: false,
    })
    .map_err(|e| e.to_string())?;
    queue.enqueue(NewTask {
        run_id: run_id.clone(),
        stage: "jd-scrape".into(),
        class: "scrape".into(),
        target: job_slug.into(),
        payload,
    })?;
    Ok(run_id)
}

/// Mark a run as `failed` with a human-readable error. The scrape step row was already
/// written by `run_scrape_step`; here we ONLY annotate its `error` field with the friendly
/// reason (no second append). A single `write_check` persists everything.
///
/// `stage` is the queue stage name of the failing scrape step (e.g. `"careers-scrape"` or
/// `"jd-scrape"`); used to locate the failed step row and, if absent, to synthesize one.
///
/// `stamp_checked` is only called for `job_check` runs (where `target` is a company slug).
/// For `job_detail` runs `target` is a job slug — stamping would corrupt a company note.
fn fail_run(
    vault_path: &str,
    run_id: &str,
    target: &str,
    stage: &str,
    error_msg: &str,
    sink: &dyn EventSink,
) {
    if let Ok(mut run) = get_check(vault_path.to_string(), run_id.to_string()) {
        // Find the last failed step for this stage (written by run_scrape_step) and annotate
        // it with the human-readable reason. If none exists (defensive), append one in-memory
        // rather than writing a second disk record.
        if let Some(step) = run.steps.iter_mut()
            .rfind(|s| s.stage == stage && s.status == "failed")
        {
            step.error = Some(error_msg.to_string());
        } else {
            run.steps.push(crate::check::Step {
                stage: stage.to_string(),
                class: "scrape".to_string(),
                target: target.to_string(),
                status: "failed".to_string(),
                attempts: 1,
                started_at: Some(now_iso()),
                finished_at: Some(now_iso()),
                error: Some(error_msg.to_string()),
                cost: None,
            });
        }
        run.status = "failed".into();
        run.errors += 1;
        run.finished_at = Some(now_iso());
        // Only stamp last_checked on job_check runs (target is a company slug).
        let kind = run.kind.clone();
        let _ = write_check(vault_path, &run);
        if kind == "job_check" {
            let today = run_id.get(..10).unwrap_or(run_id);
            stamp_checked(vault_path, target, today);
        }
    }
    // NOTE: if get_check fails we cannot determine the run kind, so we do NOT stamp.
    // Corruption of a job note (wrong kind stamped as a company) is worse than missing a stamp.
    eprintln!("run {run_id} failed: {error_msg}");
    sink.run_finished(run_id, "failed");
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
        // The run was cancelled: drop this task without dispatching.
        queue.complete(task.id)?;
        sink.step_done(&task.run_id, &task.stage, "cancelled");
        // Finalize the run exactly once. Subsequent cancelled tasks of the same run see status
        // already "cancelled" and skip the write. Do NOT stamp last_checked — cancel ≠ "we looked".
        if let Ok(mut run) = get_check(vault_path.to_string(), task.run_id.clone()) {
            if run.status != "cancelled" {
                run.status = "cancelled".into();
                run.finished_at = Some(now_iso());
                let _ = write_check(vault_path, &run);
                sink.run_finished(&task.run_id, "cancelled");
            }
        }
        return Ok(true);
    }

    // Notify the UI that this stage is now executing — emitted before dispatch so the UI can
    // display the live current-phase label immediately.
    // For scrape stages, decode the payload to pass the proxy tier as `detail`.
    if task.stage == "careers-scrape" || task.stage == "jd-scrape" {
        // --- Scrape stage: typed failure + per-class retry policy ---
        let p: ScrapePayload = serde_json::from_str(&task.payload).map_err(|e| e.to_string())?;
        let scrape_detail = if p.tier == "stealth" { Some("stealth") } else { None };
        sink.step_started(&task.run_id, &task.stage, scrape_detail);
        let tier = p.proxy_tier();
        match run_scrape_step(vault_path, &task.run_id, &p.careers_url, &task.target, &task.stage, tier, scraper) {
            Ok(scraped) => {
                let started = now_iso();
                let sanitized = sanitize(&scraped.content, &p.careers_url);
                record_step(vault_path, &task.run_id, "sanitize", "script", &task.target, started, "ok", None, None)?;

                // For jd-scrape: write the raw JD to jobs/_jd/<slug>.md and set jd_raw_file.
                if task.stage == "jd-scrape" {
                    let jd_dir = Path::new(vault_path).join("jobs").join("_jd");
                    std::fs::create_dir_all(&jd_dir).map_err(|e| e.to_string())?;
                    let jd_raw_path = jd_dir.join(format!("{}.md", task.target));
                    crate::note::write_note(&jd_raw_path, &scraped.content)
                        .map_err(|e| format!("write jd_raw_file: {e}"))?;
                    let rel_path = format!("jobs/_jd/{}.md", task.target);
                    update_job_field(
                        vault_path.to_string(),
                        task.target.clone(),
                        "jd_raw_file".into(),
                        rel_path,
                    )?;
                }

                // Choose the successor stage based on which scrape just ran.
                let (succ_stage, succ_payload) = if task.stage == "jd-scrape" {
                    (
                        "structure-jd",
                        serde_json::to_string(&JdStructurePayload {
                            slug: task.target.clone(),
                            sanitized,
                        })
                        .map_err(|e| e.to_string())?,
                    )
                } else {
                    (
                        "structure-listings",
                        serde_json::to_string(&StructurePayload { sanitized })
                            .map_err(|e| e.to_string())?,
                    )
                };
                queue.enqueue(NewTask {
                    run_id: task.run_id.clone(),
                    stage: succ_stage.into(),
                    class: "llm".into(),
                    target: task.target.clone(),
                    payload: succ_payload,
                })?;
                queue.complete(task.id)?;
                sink.step_done(&task.run_id, &task.stage, "ok");
            }
            Err(scrape_err) => {
                sink.step_done(&task.run_id, &task.stage, "failed");
                match scrape_err.class {
                    FailureClass::Terminal => {
                        // Page is gone (or API key missing) — no retry, no escalation, no spend.
                        let reason = match scrape_err.status {
                            Some(404) => "page not found (404)".to_string(),
                            Some(410) => "page gone (410)".to_string(),
                            Some(s) => format!("terminal error ({s})"),
                            None => {
                                // No HTTP status: the failure happened before the request (e.g. missing
                                // API key). Derive the reason from the real body; detect missing-key case.
                                let body = &scrape_err.body;
                                if body.to_lowercase().contains("scrapingbee_api_key")
                                    || body.to_lowercase().contains("no entry")
                                    || body.to_lowercase().contains("not found")
                                {
                                    "ScrapingBee API key not set — add it in Settings".to_string()
                                } else {
                                    body.clone()
                                }
                            }
                        };
                        queue.kill(task.id, &reason)?;
                        fail_run(vault_path, &task.run_id, &task.target, &task.stage, &reason, sink);
                    }
                    FailureClass::FixEncoding => {
                        if p.encoding_fixed {
                            // Already re-issued with encoded URL — still failing. Give up.
                            let reason = "url encoding fix did not resolve the error";
                            queue.kill(task.id, reason)?;
                            fail_run(vault_path, &task.run_id, &task.target, &task.stage, reason, sink);
                        } else {
                            // Re-issue once with RFC-3986-percent-encoded URL.
                            let fixed_url = percent_encode_target_url(&p.careers_url);
                            let new_payload = serde_json::to_string(&ScrapePayload {
                                careers_url: fixed_url,
                                tier: p.tier.clone(),
                                encoding_fixed: true,
                            })
                            .map_err(|e| e.to_string())?;
                            queue.kill(task.id, "re-issuing with encoded url")?;
                            queue.enqueue(NewTask {
                                run_id: task.run_id.clone(),
                                stage: task.stage.clone(), // re-enqueue as the SAME stage
                                class: "scrape".into(),
                                target: task.target.clone(),
                                payload: new_payload,
                            })?;
                        }
                    }
                    FailureClass::EscalateProxy => {
                        if p.tier == "stealth" {
                            // Already escalated to Stealth — still blocked. Give up.
                            let reason = "blocked — escalated to stealth, still failed";
                            queue.kill(task.id, reason)?;
                            fail_run(vault_path, &task.run_id, &task.target, &task.stage, reason, sink);
                        } else {
                            // Re-enqueue once with Stealth tier.
                            let new_payload = serde_json::to_string(&ScrapePayload {
                                careers_url: p.careers_url.clone(),
                                tier: "stealth".into(),
                                encoding_fixed: p.encoding_fixed,
                            })
                            .map_err(|e| e.to_string())?;
                            queue.kill(task.id, "escalating to stealth proxy")?;
                            queue.enqueue(NewTask {
                                run_id: task.run_id.clone(),
                                stage: task.stage.clone(), // re-enqueue as the SAME stage
                                class: "scrape".into(),
                                target: task.target.clone(),
                                payload: new_payload,
                            })?;
                        }
                    }
                    FailureClass::Transient => {
                        // Bounded backoff retry, capped at TRANSIENT_SCRAPE_MAX_ATTEMPTS.
                        let err_str = scrape_err.to_string();
                        if task.attempts >= TRANSIENT_SCRAPE_MAX_ATTEMPTS {
                            queue.kill(task.id, &err_str)?;
                            fail_run(
                                vault_path, &task.run_id, &task.target, &task.stage,
                                &format!("transient scrape error after {TRANSIENT_SCRAPE_MAX_ATTEMPTS} attempts: {err_str}"),
                                sink,
                            );
                        } else {
                            queue.fail(task.id, &err_str)?;
                        }
                    }
                }
            }
        }
        return Ok(true);
    }

    // --- Non-scrape stages (LLM, script): simple bounded retry via MAX_ATTEMPTS ---
    sink.step_started(&task.run_id, &task.stage, None);
    match dispatch_non_scrape(&task, vault_path, cfg, llm) {
        Ok(successors) => {
            for s in successors {
                queue.enqueue(s)?;
            }
            queue.complete(task.id)?;
            sink.step_done(&task.run_id, &task.stage, "ok");
            // Emit run-complete for both terminal stages of their respective chains.
            if matches!(task.stage.as_str(), "finalize" | "alignment") {
                sink.run_finished(&task.run_id, "complete");
            }
        }
        Err(e) => {
            queue.fail(task.id, &e)?;
            sink.step_done(&task.run_id, &task.stage, "failed");
            // Terminal failure (retries exhausted): the chain can't proceed — mark the run failed
            // so it doesn't sit "running" forever.
            if task.attempts >= MAX_ATTEMPTS {
                // Update the check note if it can be loaded (legitimately needs the loaded check).
                if let Ok(mut run) = get_check(vault_path.to_string(), task.run_id.clone()) {
                    run.status = "failed".into();
                    run.errors += 1;
                    run.finished_at = Some(now_iso());
                    let _ = write_check(vault_path, &run);
                    // Only stamp last_checked for job_check runs (target is a company slug).
                    // For job_detail runs, target is a job slug — stamping would corrupt/create
                    // a wrong company note.
                    if run.kind == "job_check" {
                        let today = task.run_id.get(..10).unwrap_or(&task.run_id);
                        stamp_checked(vault_path, &task.target, today);
                    }
                }
                sink.run_finished(&task.run_id, "failed");
            }
        }
    }
    Ok(true)
}

/// Execute one pipeline step for non-scrape stages (structure-listings, finalize).
/// Returns the successor task(s) to enqueue (empty when the chain ends).
/// Records the step's telemetry into the `checks/` note as a side effect.
fn dispatch_non_scrape<L: Llm>(
    task: &QueuedTask,
    vault_path: &str,
    cfg: &PipelineConfig,
    llm: &L,
) -> Result<Vec<NewTask>, String> {
    let run_id = task.run_id.as_str();
    let company = task.target.as_str();
    match task.stage.as_str() {
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
                let validated_level = listing.level.as_deref()
                    .and_then(|v| if crate::job::VALID_LEVELS.contains(&v) { Some(v.to_string()) } else { None });
                let job = Job {
                    slug: job_slug(&listing.title, company),
                    title: listing.title.clone(),
                    company: Some(company.to_string()),
                    url: listing.url.clone(),
                    level: validated_level,
                    location: listing.location.clone(),
                    comp_low: None,
                    comp_high: None,
                    comp_currency: None,
                    comp_raw: None,
                    comp_period: None,
                    comp_equity: None,
                    employment_type: None,
                    yoe_min: None,
                    yoe_max: None,
                    required_skills: vec![],
                    preferred_skills: vec![],
                    reports_to: None,
                    team: None,
                    remote: None,
                    location_constraints: None,
                    visa_sponsorship: None,
                    relocation: None,
                    countries: vec![],
                    metros: vec![],
                    application_url: None,
                    date_posted: None,
                    last_seen: Some(today.to_string()),
                    ats: listing.ats.clone(),
                    tech_stack: vec![],
                    fit_score: None,
                    researched: vec![],
                    status: Some("new".to_string()),
                    skip_reason: None,
                    jd_raw_file: None,
                    jd_fetched: false,
                };
                if let Err(e) = write_job_stub(vault_path, &job) {
                    eprintln!("failed to write stub {}: {e}", job.slug);
                }
            }

            let mut run = get_check(vault_path.to_string(), run_id.to_string())?;
            run.roles_found = selected.len() as u32;
            run.status = "complete".into();
            run.finished_at = Some(now_iso());
            write_check(vault_path, &run)?;
            stamp_checked(vault_path, company, today);
            Ok(vec![])
        }
        "structure-jd" => {
            let p: JdStructurePayload =
                serde_json::from_str(&task.payload).map_err(|e| e.to_string())?;
            let started = now_iso();
            let prompt = build_structure_jd_prompt(&model_for(cfg, "structure-jd"), &p.sanitized);
            let resp = llm.complete(&prompt);
            let (jd, cost) = match resp {
                Ok(r) => match parse_structured_jd(&r.content) {
                    Ok(jd) => (jd, r.cost_micro_usd),
                    Err(e) => {
                        record_step(vault_path, run_id, "structure-jd", "llm", &p.slug, started, "failed", Some(e.clone()), r.cost_micro_usd)?;
                        return Err(e);
                    }
                },
                Err(e) => {
                    record_step(vault_path, run_id, "structure-jd", "llm", &p.slug, started, "failed", Some(e.clone()), None)?;
                    return Err(e);
                }
            };
            write_jd_fields(vault_path, &p.slug, &jd)?;
            record_step(vault_path, run_id, "structure-jd", "llm", &p.slug, started, "ok", None, cost)?;
            Ok(vec![NewTask {
                run_id: run_id.into(),
                stage: "gap-detect".into(),
                class: "script".into(),
                target: p.slug,
                payload: "{}".into(),
            }])
        }
        other => Err(format!("unknown stage: {other}")),
    }
}

/// Write structured JD fields to the job note. For each populated scalar field, calls
/// `update_job_field` (which validates enum values). Enum-constrained fields are pre-validated
/// against their constant sets; invalid values are skipped with a warning so that one stray
/// LLM output value doesn't fail the entire stage. List fields use `set_job_list_field`.
/// Assembles and writes the candidate-brief `## JD — structured` body section.
fn write_jd_fields(vault_path: &str, slug: &str, jd: &StructuredJd) -> Result<(), String> {
    // Helper: write a scalar field, propagating real IO errors but skipping invalid enum values.
    let write_scalar = |field: &str, value: &str| -> Result<(), String> {
        update_job_field(vault_path.to_string(), slug.to_string(), field.into(), value.into())
    };

    // Enum-constrained fields: pre-validate, skip + warn on bad value, propagate IO errors.
    macro_rules! write_enum {
        ($field:expr, $value:expr, $allowed:expr) => {
            if let Some(v) = $value.as_deref() {
                if $allowed.contains(&v) {
                    write_scalar($field, v)?;
                } else {
                    eprintln!("structure-jd: invalid {field:?} value {v:?} (skipped)", field = $field);
                }
            }
        };
    }

    write_enum!("remote", &jd.remote, REMOTE_KINDS);
    write_enum!("employment_type", &jd.employment_type, EMPLOYMENT_TYPES);
    write_enum!("visa_sponsorship", &jd.visa_sponsorship, SPONSORSHIP);
    write_enum!("relocation", &jd.relocation, SPONSORSHIP);
    write_enum!("level", &jd.level, VALID_LEVELS);

    // Free-text scalar fields.
    if let Some(v) = jd.comp_low { write_scalar("comp_low", &v.to_string())?; }
    if let Some(v) = jd.comp_high { write_scalar("comp_high", &v.to_string())?; }
    if let Some(v) = jd.comp_currency.as_deref() { write_scalar("comp_currency", v)?; }
    if let Some(v) = jd.comp_period.as_deref() { write_scalar("comp_period", v)?; }
    if let Some(v) = jd.comp_equity.as_deref() { write_scalar("comp_equity", v)?; }
    if let Some(v) = jd.yoe_min { write_scalar("yoe_min", &v.to_string())?; }
    if let Some(v) = jd.yoe_max { write_scalar("yoe_max", &v.to_string())?; }
    if let Some(v) = jd.reports_to.as_deref() { write_scalar("reports_to", v)?; }
    if let Some(v) = jd.team.as_deref() { write_scalar("team", v)?; }
    if let Some(v) = jd.location_constraints.as_deref() { write_scalar("location_constraints", v)?; }
    if let Some(v) = jd.application_url.as_deref() { write_scalar("application_url", v)?; }
    if let Some(v) = jd.date_posted.as_deref() { write_scalar("date_posted", v)?; }

    // List fields.
    if !jd.tech_stack.is_empty() {
        set_job_list_field(vault_path.to_string(), slug.to_string(), "tech_stack".into(), jd.tech_stack.clone())?;
    }
    if !jd.required_skills.is_empty() {
        set_job_list_field(vault_path.to_string(), slug.to_string(), "required_skills".into(), jd.required_skills.clone())?;
    }
    if !jd.preferred_skills.is_empty() {
        set_job_list_field(vault_path.to_string(), slug.to_string(), "preferred_skills".into(), jd.preferred_skills.clone())?;
    }

    // Candidate-brief body section.
    let mut brief_parts: Vec<String> = Vec::new();
    if let Some(v) = jd.role_brief.as_deref() {
        brief_parts.push(v.to_string());
    }
    if let Some(v) = jd.must_haves.as_deref() {
        brief_parts.push(format!("**Must-haves:** {v}"));
    }
    if let Some(v) = jd.nice_to_haves.as_deref() {
        brief_parts.push(format!("**Nice-to-haves:** {v}"));
    }
    if let Some(v) = jd.signals.as_deref() {
        brief_parts.push(format!("**Signals:** {v}"));
    }
    if let Some(v) = jd.open_questions.as_deref() {
        brief_parts.push(format!("**Open questions:** {v}"));
    }
    if !brief_parts.is_empty() {
        let brief = brief_parts.join("\n\n");
        set_job_section(vault_path, slug, "## JD — structured", &brief)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::llm::{Llm, LlmRequest, LlmResponse};
    use crate::pipeline::queue::SqliteQueue;
    use crate::scraper::{FailureClass, ProxyTier, ScrapeError, ScrapeResult, Scraper};
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
            "---\ntype: target_criteria\nwork_arrangements: [remote]\nmatch_titles:\n  - engineer\n---\n",
        )
        .unwrap();
        let vault = dir.to_str().unwrap().to_string();
        (dir, vault)
    }

    fn two_listings() -> String {
        r#"[{"title":"Senior Engineer","url":"https://co/1","location":"Remote","level":"senior"},
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
        fn fetch(&self, _url: &str, _tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
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

    /// Scraper that always fails with a given `FailureClass`.
    struct ClassFailScraper {
        class: FailureClass,
        status: Option<u16>,
        calls: Cell<u32>,
        /// If set, records which tier was used for each call.
        last_tier: Cell<Option<ProxyTier>>,
    }
    impl ClassFailScraper {
        fn new(class: FailureClass, status: Option<u16>) -> Self {
            Self { class, status, calls: Cell::new(0), last_tier: Cell::new(None) }
        }
    }
    impl Scraper for ClassFailScraper {
        fn fetch(&self, _url: &str, tier: ProxyTier) -> Result<ScrapeResult, ScrapeError> {
            self.calls.set(self.calls.get() + 1);
            self.last_tier.set(Some(tier));
            Err(ScrapeError {
                status: self.status,
                body: "fake failure".into(),
                class: self.class.clone(),
            })
        }
    }

    struct AlwaysOkLlm {
        reply: String,
    }
    impl Llm for AlwaysOkLlm {
        fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse, String> {
            Ok(LlmResponse { content: self.reply.clone(), cost_micro_usd: Some(10_000) })
        }
    }

    #[test]
    fn discovery_drains_to_filtered_stubs_and_complete() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = FlakyLlm { reply: two_listings(), calls: Cell::new(1) }; // start at 1 -> always succeed

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "complete");
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
        assert_eq!(run.status, "complete"); // recovered to completion
        assert_eq!(run.roles_found, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn terminal_failure_does_not_retry() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = ClassFailScraper::new(FailureClass::Terminal, Some(404));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // Scraper called exactly once — Terminal = no retry
        assert_eq!(scraper.calls.get(), 1, "Terminal must not retry");
        let run = get_check(vault.clone(), run_id.clone()).unwrap();
        assert_eq!(run.status, "failed", "run must be marked failed");
        // Queue is fully drained (no pending tasks remain)
        assert_eq!(q.pending_count().unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn terminal_failure_records_human_reason_in_step() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        // 404 → Terminal → reason = "page not found (404)"
        let scraper = ClassFailScraper::new(FailureClass::Terminal, Some(404));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "failed", "run must be marked failed");

        // There must be EXACTLY ONE failed careers-scrape step (no duplicate from fail_run).
        let failed_scrape_steps: Vec<_> = run.steps.iter()
            .filter(|s| s.stage == "careers-scrape" && s.status == "failed")
            .collect();
        assert_eq!(
            failed_scrape_steps.len(), 1,
            "must have exactly ONE failed careers-scrape step (no duplicate); steps: {:?}", run.steps
        );
        assert_eq!(run.errors, 1, "errors counter must be 1 (failed_count matches errors)");

        // The single step must carry the human reason.
        let err = failed_scrape_steps[0].error.as_deref().unwrap_or("");
        assert!(
            err.contains("404"),
            "step error must contain the human reason (e.g. '404'), got: {err:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn block_escalates_to_stealth_once() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = ClassFailScraper::new(FailureClass::EscalateProxy, Some(500));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // Called twice: once with Premium, once with Stealth
        assert_eq!(scraper.calls.get(), 2, "EscalateProxy must try twice (Premium then Stealth)");
        // The last call used Stealth
        assert_eq!(
            scraper.last_tier.get(),
            Some(ProxyTier::Stealth),
            "second call must use Stealth tier"
        );
        // Both failed → run is marked failed
        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "failed", "run must be failed after stealth also blocked");
        assert_eq!(q.pending_count().unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn transient_retries_bounded_by_transient_cap() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = ClassFailScraper::new(FailureClass::Transient, Some(429));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // Should retry up to TRANSIENT_SCRAPE_MAX_ATTEMPTS (2), then fail
        assert_eq!(
            scraper.calls.get(),
            TRANSIENT_SCRAPE_MAX_ATTEMPTS,
            "Transient scrape retries exactly TRANSIENT_SCRAPE_MAX_ATTEMPTS times"
        );
        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "failed");
        assert_eq!(q.pending_count().unwrap(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fix_encoding_re_issues_once_then_terminal_if_still_failing() {
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = ClassFailScraper::new(FailureClass::FixEncoding, Some(500));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers?q=a+b", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // Called twice: original URL, then re-issued with encoded URL
        assert_eq!(scraper.calls.get(), 2, "FixEncoding must re-issue once");
        let run = get_check(vault, run_id).unwrap();
        assert_eq!(run.status, "failed", "run must fail if encoding fix didn't help");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn finalize_validates_level_drops_unknown_keeps_valid() {
        // level:"senior" → Some("senior"); level:"wizard" → None (invalid value dropped).
        let (dir, vault) = setup_vault();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        // Two listings: one with a valid level, one with an invalid level
        let reply = r#"[{"title":"Senior Engineer","url":"https://co/1","level":"senior"},
                        {"title":"Wizard","url":"https://co/2","level":"wizard"}]"#.to_string();
        // Both pass the title filter (target_criteria has "engineer" so only "Senior Engineer" makes it).
        // We need a title that passes the filter for the wizard one too — use a separate profile.
        // Actually, to test level validation specifically, use a profile that matches both.
        std::fs::write(
            dir.join("profile/target_criteria.md"),
            "---\ntype: target_criteria\nwork_arrangements: [remote]\nmatch_titles:\n  - engineer\n  - wizard\n---\n",
        ).unwrap();
        let llm = AlwaysOkLlm { reply };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-18").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault.clone(), run_id).unwrap();
        assert_eq!(run.status, "complete");
        // Both stubs should be written
        let senior_path = dir.join("jobs/senior-engineer-acme.md");
        let wizard_path = dir.join("jobs/wizard-acme.md");
        assert!(senior_path.exists(), "senior engineer stub should be written");
        assert!(wizard_path.exists(), "wizard stub should be written");
        // Parse and check level values
        let senior_text = std::fs::read_to_string(&senior_path).unwrap();
        let wizard_text = std::fs::read_to_string(&wizard_path).unwrap();
        assert!(senior_text.contains("level: senior"), "valid level 'senior' must be written");
        assert!(!wizard_text.contains("level:"), "invalid level 'wizard' must be dropped (None)");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// After a successful drain `last_checked` is stamped on the target company note.
    #[test]
    fn successful_run_stamps_last_checked() {
        let (dir, vault) = setup_vault();
        // Create a company note so stamp_checked has something to update.
        std::fs::create_dir_all(dir.join("companies")).unwrap();
        std::fs::write(
            dir.join("companies/acme.md"),
            "---\nid: acme\nname: Acme\nstatus: active\n---\n",
        ).unwrap();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-19").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault.clone(), run_id.clone()).unwrap();
        assert_eq!(run.status, "complete");

        // The company note must now contain `last_checked: 2026-06-19` (the run's date prefix).
        let company_text = std::fs::read_to_string(dir.join("companies/acme.md")).unwrap();
        assert!(
            company_text.contains("last_checked: 2026-06-19"),
            "successful run must stamp last_checked; got:\n{company_text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── job_detail chain tests ────────────────────────────────────────────────────────────────────

    /// Write a minimal job-detail fixture vault with one job stub (url + company) and a
    /// target_criteria profile. Returns the temp dir (caller must remove_dir_all).
    fn job_detail_fixture_vault() -> std::path::PathBuf {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join(format!("lodestar-jd-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        std::fs::create_dir_all(dir.join("profile")).unwrap();
        // Minimal job stub with url + company so start_job_detail can resolve both.
        std::fs::write(
            dir.join("jobs/senior-engineer-acme.md"),
            "---\nid: senior-engineer-acme\ntitle: \"Senior Engineer\"\ncompany: \"[[acme]]\"\nurl: https://acme.com/jobs/1\nstatus: new\n---\n",
        ).unwrap();
        std::fs::write(
            dir.join("profile/target_criteria.md"),
            "---\ntype: target_criteria\nwork_arrangements: [remote]\nmatch_titles:\n  - engineer\n---\n",
        ).unwrap();
        dir
    }

    #[test]
    fn job_detail_structure_jd_writes_fields_from_fake_llm() {
        use crate::llm::tests::FakeLlm;
        use crate::scraper::tests::FakeScraper;

        // Arrange: a vault with one job stub (has url + company), a target_criteria, fakes.
        let dir = job_detail_fixture_vault();
        let vault = dir.to_str().unwrap();
        let queue = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let run_id = start_job_detail(&queue, vault, "senior-engineer-acme", "2026-06-19").unwrap();

        let scraper = FakeScraper { content: "<p>JD</p>".into(), credits: 5 };
        let llm = FakeLlm {
            // structure-jd returns a JSON object
            reply: r#"{"comp_low":180000,"comp_high":220000,"comp_currency":"USD","comp_period":"annual",
              "required_skills":["rust"],"preferred_skills":["kubernetes"],"remote":"remote",
              "role_brief":"Build platform.","must_haves":"5y","nice_to_haves":"k8s","signals":"early","open_questions":"on-call?"}"#.into(),
            cost_micro_usd: 1000,
        };
        let cfg = default_config();
        let sink = NoopSink;
        let never = |_: &str| false;
        // Pump: jd-scrape (scrape+sanitize) then structure-jd. Gap-detect is not yet implemented,
        // so dispatch_non_scrape returns "unknown stage: gap-detect" and the loop terminates.
        while pump_once(&queue, vault, &cfg, &scraper, &llm, &sink, &never).unwrap() {}

        let j = crate::job::parse_job("senior-engineer-acme",
            &std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap()).unwrap();
        assert_eq!(j.comp_low, Some(180000));
        assert_eq!(j.required_skills, vec!["rust"]);
        assert_eq!(j.remote.as_deref(), Some("remote"));
        let body = std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap();
        assert!(body.contains("## JD — structured") && body.contains("Build platform."),
            "expected '## JD — structured' and 'Build platform.' in note:\n{body}");
        // The raw JD file must be written.
        assert!(dir.join("jobs/_jd/senior-engineer-acme.md").exists(),
            "jd_raw_file must be written at jobs/_jd/senior-engineer-acme.md");
        // run_id must be a valid run id string.
        assert!(!run_id.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// After a terminal-failure drain `last_checked` is ALSO stamped on the target company note.
    #[test]
    fn terminal_failure_stamps_last_checked() {
        let (dir, vault) = setup_vault();
        std::fs::create_dir_all(dir.join("companies")).unwrap();
        std::fs::write(
            dir.join("companies/acme.md"),
            "---\nid: acme\nname: Acme\nstatus: active\n---\n",
        ).unwrap();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let scraper = ClassFailScraper::new(FailureClass::Terminal, Some(404));
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-19").unwrap();
        drain(&q, &vault, &scraper, &llm);

        let run = get_check(vault.clone(), run_id.clone()).unwrap();
        assert_eq!(run.status, "failed");

        // last_checked must be stamped even on terminal failure.
        let company_text = std::fs::read_to_string(dir.join("companies/acme.md")).unwrap();
        assert!(
            company_text.contains("last_checked: 2026-06-19"),
            "terminal-failure run must stamp last_checked; got:\n{company_text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Cancellation finalizes the run as `cancelled` and must NOT stamp `last_checked`.
    #[test]
    fn cancel_finalizes_run_and_does_not_stamp_last_checked() {
        let (dir, vault) = setup_vault();
        std::fs::create_dir_all(dir.join("companies")).unwrap();
        std::fs::write(
            dir.join("companies/acme.md"),
            "---\nid: acme\nname: Acme\nstatus: active\n---\n",
        )
        .unwrap();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        // Scraper would succeed but we never reach it — the run is cancelled immediately.
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = AlwaysOkLlm { reply: two_listings() };

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-19").unwrap();
        let cfg = default_config();
        // is_cancelled returns true for every run_id → every task is cancelled immediately.
        while pump_once(&q, &vault, &cfg, &scraper, &llm, &NoopSink, &|_| true).unwrap() {}

        let run = get_check(vault.clone(), run_id.clone()).unwrap();
        assert_eq!(run.status, "cancelled", "run must be finalised as cancelled; got {:?}", run.status);

        // last_checked must NOT be set — cancel does not count as a check.
        let company_text = std::fs::read_to_string(dir.join("companies/acme.md")).unwrap();
        assert!(
            !company_text.contains("last_checked"),
            "cancel must NOT stamp last_checked; got:\n{company_text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// LLM that always returns an error — drives the non-scrape retry-exhausted path.
    struct AlwaysFailLlm;
    impl Llm for AlwaysFailLlm {
        fn complete(&self, _req: &LlmRequest) -> Result<LlmResponse, String> {
            Err("llm unavailable".into())
        }
    }

    /// When the LLM stage exhausts MAX_ATTEMPTS on a `job_check` run, `last_checked` is stamped
    /// on the target company note (when `get_check` succeeds) and the run is marked `failed`.
    #[test]
    fn llm_exhausted_stamps_last_checked() {
        let (dir, vault) = setup_vault();
        // Create a company note so stamp_checked has something to update.
        std::fs::create_dir_all(dir.join("companies")).unwrap();
        std::fs::write(
            dir.join("companies/acme.md"),
            "---\nid: acme\nname: Acme\nstatus: active\n---\n",
        ).unwrap();
        let q = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        // Scraper succeeds so we reach the LLM stage.
        let scraper = CountingScraper { content: "<p>careers</p>".into(), credits: 5, calls: Cell::new(0) };
        let llm = AlwaysFailLlm;

        let run_id = start_discovery(&q, &vault, "acme", "https://co/careers", "2026-06-19").unwrap();
        drain(&q, &vault, &scraper, &llm);

        // Run must be marked failed (not left stuck in "running").
        let run = get_check(vault.clone(), run_id.clone()).unwrap();
        assert_eq!(run.status, "failed", "LLM-exhausted run must be marked failed; got {:?}", run.status);

        // last_checked must be stamped with the run's date prefix.
        let company_text = std::fs::read_to_string(dir.join("companies/acme.md")).unwrap();
        assert!(
            company_text.contains("last_checked: 2026-06-19"),
            "LLM-exhausted run must stamp last_checked; got:\n{company_text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A `job_detail` run that exhausts MAX_ATTEMPTS at the LLM stage must NOT stamp
    /// `last_checked` on any company note, and must NOT create a spurious note at
    /// `companies/<job-slug>.md` (the bug that would result if the kind-gate were absent).
    #[test]
    fn job_detail_terminal_failure_does_not_stamp_company() {
        use crate::scraper::tests::FakeScraper;

        // Arrange: vault with a job stub (url + company) and a real company note.
        let dir = job_detail_fixture_vault();
        std::fs::create_dir_all(dir.join("companies")).unwrap();
        std::fs::write(
            dir.join("companies/acme.md"),
            "---\nid: acme\nname: Acme\nstatus: active\n---\n",
        ).unwrap();
        let vault = dir.to_str().unwrap();
        let queue = SqliteQueue::open(&dir.join("queue.db")).unwrap();
        let run_id = start_job_detail(&queue, vault, "senior-engineer-acme", "2026-06-20").unwrap();

        // Scraper succeeds (so we reach the LLM stage), LLM always fails (exhausts MAX_ATTEMPTS).
        let scraper = FakeScraper { content: "<p>JD content</p>".into(), credits: 5 };
        let llm = AlwaysFailLlm;
        drain(&queue, vault, &scraper, &llm);

        // Run must end as failed (not stuck in running).
        let run = get_check(vault.to_string(), run_id.clone()).unwrap();
        assert_eq!(run.status, "failed", "LLM-exhausted job_detail run must be marked failed; got {:?}", run.status);

        // The kind-gate must hold: acme.md must have NO last_checked field.
        let company_text = std::fs::read_to_string(dir.join("companies/acme.md")).unwrap();
        assert!(
            !company_text.contains("last_checked"),
            "job_detail run must NOT stamp last_checked on company note; got:\n{company_text}"
        );
        // No spurious companies/<job-slug>.md note must have been created.
        assert!(
            !dir.join("companies/senior-engineer-acme.md").exists(),
            "job_detail run must NOT create a company note named after the job slug"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
