//! Tauri integration for the job-fetch pipeline: shared state (the durable queue + the
//! cancellation set), the live-progress event sink, the background drain worker, and the
//! `fetch_jobs_for_company` / `cancel_run` commands.
//!
//! The drain runs on a `std::thread` — deliberately OFF the tokio reactor — so the real
//! `ScrapingBeeScraper`/`OpenRouterLlm` (`reqwest::blocking`) are sound. The app always wires
//! those real impls; there is no fake path here.

use crate::company::list_companies;
use crate::config::load_config;
use crate::llm::OpenRouterLlm;
use crate::pipeline::queue::SqliteQueue;
use crate::pipeline::steps::{
    abort_running_runs, abort_set, pump_once, start_discovery, start_job_detail_runs,
    start_rescore_run, EventSink, FetchJobDetailsOutcome,
};
use crate::scraper::ScrapingBeeScraper;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

/// Shared pipeline state, managed by Tauri and shared with the drain threads.
pub struct PipelineState {
    pub queue: Arc<SqliteQueue>,
    pub cancelled: Arc<Mutex<HashSet<String>>>,
}

#[derive(Clone, Serialize)]
struct StepEvent {
    run_id: String,
    /// The single entity this run is about — a job slug for `job_detail`/`job_scoring`, a company
    /// slug for `job_check`. Lets the frontend track a run (and both runs of a fetch→score handoff)
    /// by the job/company without a separate lookup.
    subject: String,
    stage: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Emits live progress to the frontend via Tauri events, and prunes a run's id from the shared
/// `cancelled` set when it finishes (so the set doesn't grow unbounded across a long session).
struct TauriSink {
    app: AppHandle,
    cancelled: Arc<Mutex<HashSet<String>>>,
}
impl EventSink for TauriSink {
    fn step_done(&self, run_id: &str, subject: &str, stage: &str, status: &str) {
        let _ = self.app.emit(
            "run:step",
            StepEvent {
                run_id: run_id.into(),
                subject: subject.into(),
                stage: stage.into(),
                status: status.into(),
                detail: None,
            },
        );
    }
    fn run_finished(&self, run_id: &str, subject: &str, status: &str) {
        let _ = self.app.emit(
            "run:finished",
            StepEvent {
                run_id: run_id.into(),
                subject: subject.into(),
                stage: String::new(),
                status: status.into(),
                detail: None,
            },
        );
        // A finished run never needs cancelling again — drop its id so the set stays bounded.
        if let Ok(mut set) = self.cancelled.lock() {
            set.remove(run_id);
        }
    }
    fn step_started(&self, run_id: &str, subject: &str, stage: &str, detail: Option<&str>) {
        let _ = self.app.emit(
            "run:step",
            StepEvent {
                run_id: run_id.into(),
                subject: subject.into(),
                stage: stage.into(),
                status: "running".into(),
                detail: detail.map(|s| s.to_string()),
            },
        );
    }
}

/// Start a discovery run for one company and drain it on a background thread. Returns the run id
/// immediately; progress streams via `run:step` / `run:finished` events.
#[tauri::command]
pub fn fetch_jobs_for_company(
    app: AppHandle,
    state: State<'_, PipelineState>,
    vault_path: String,
    slug: String,
) -> Result<String, String> {
    let careers_url = list_companies(vault_path.clone())?
        .iter()
        .find(|c| c.slug == slug)
        .and_then(|c| c.careers_url.clone())
        .ok_or_else(|| format!("company {slug:?} has no careers_url to scrape"))?;

    let cfg = load_config(&app.path().app_config_dir().map_err(|e| e.to_string())?);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let run_id = start_discovery(&*state.queue, &vault_path, &slug, &careers_url, &today)?;

    spawn_drain(app, &state, vault_path, cfg, vec![run_id.clone()]);
    Ok(run_id)
}

/// Drain the queue on a background thread (off the tokio reactor — the real scraper/LLM are
/// blocking HTTP). Loops `pump_once` until the queue empties. On a non-per-step pump error the
/// drain can't continue safely, so it ABORTS the runs it owns that are still `running` — plus any
/// `job_scoring` run the handoff spawned for an owned subject (`abort_set`) — marking them failed +
/// discarding their tasks, emits a `run:finished`=failed for each so the UI isn't blind, and stops.
/// The durable queue means a clean crash leaves tasks persisted for a later drain; this path
/// handles the case where the drain itself errors out.
fn spawn_drain(
    app: AppHandle,
    state: &State<'_, PipelineState>,
    vault_path: String,
    cfg: crate::config::PipelineConfig,
    owned_run_ids: Vec<String>,
) {
    let queue = state.queue.clone();
    let cancelled = state.cancelled.clone();
    std::thread::spawn(move || {
        let scraper = ScrapingBeeScraper;
        let llm = OpenRouterLlm;
        let sink = TauriSink { app, cancelled: cancelled.clone() };
        let is_cancelled =
            move |rid: &str| cancelled.lock().map(|set| set.contains(rid)).unwrap_or(false);
        loop {
            match pump_once(&*queue, &vault_path, &cfg, &scraper, &llm, &sink, &is_cancelled) {
                Ok(true) => continue,
                Ok(false) => break, // queue drained
                Err(e) => {
                    eprintln!("pipeline worker error: {e}");
                    let reason = format!("fetch worker stopped after a task errored: {e}");
                    let to_abort = abort_set(&vault_path, &owned_run_ids);
                    for id in abort_running_runs(&*queue, &vault_path, &to_abort, &reason) {
                        // Look up the run's subject for the event (the run note is the source of
                        // truth); fall back to the run id if it can't be read.
                        let subject = crate::check::get_check(vault_path.clone(), id.clone())
                            .map(|c| c.subject)
                            .unwrap_or_else(|_| id.clone());
                        sink.run_finished(&id, &subject, "failed");
                    }
                    break;
                }
            }
        }
    });
}

/// Start a `job_detail` run for each selected job (the Roles tab's "Fetch selected") and drain
/// them on one background thread. Returns immediately with the per-slug outcome — which jobs
/// opened runs (with run ids, for live-progress attribution) and which failed to start (with the
/// reason) — never eprintln-swallowing a bad pick. Progress streams via `run:step`/`run:finished`.
#[tauri::command]
pub fn fetch_job_details(
    app: AppHandle,
    state: State<'_, PipelineState>,
    vault_path: String,
    slugs: Vec<String>,
) -> Result<FetchJobDetailsOutcome, String> {
    let cfg = load_config(&app.path().app_config_dir().map_err(|e| e.to_string())?);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let outcome = start_job_detail_runs(&*state.queue, &vault_path, &slugs, &today);

    // Nothing started → nothing to drain.
    if outcome.started.is_empty() {
        return Ok(outcome);
    }

    // One drain thread for all enqueued runs (the shared durable queue means two concurrent drains
    // just split the work). It owns the runs it started — those are what it aborts on a drain error.
    let owned: Vec<String> = outcome.started.iter().map(|r| r.run_id.clone()).collect();
    spawn_drain(app, &state, vault_path, cfg, owned);
    Ok(outcome)
}

/// Re-score one job: open a fresh `job_scoring` run (fit-score → alignment) and drain it on a
/// background thread. Returns the new run id immediately; progress streams via `run:step` /
/// `run:finished`. Scoring reads the job's CURRENT persisted fields — no scrape, no structure-jd —
/// so a re-score after a manual edit refreshes the score/narrative WITHOUT clobbering the edit.
///
/// Guards live in `start_rescore_run`: a missing job → `Err`; a `job_detail` OR `job_scoring` run
/// already in flight for this slug → `Err` (don't stack). Decided jobs (selected/applied/skipped)
/// ARE allowed — `fit-score`'s `advance_job_status` no-ops on a decision, so the status is kept.
#[tauri::command]
pub fn rescore_job(
    app: AppHandle,
    state: State<'_, PipelineState>,
    vault_path: String,
    slug: String,
) -> Result<String, String> {
    let cfg = load_config(&app.path().app_config_dir().map_err(|e| e.to_string())?);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let run_id = start_rescore_run(&*state.queue, &vault_path, &slug, &today)?;
    spawn_drain(app, &state, vault_path, cfg, vec![run_id.clone()]);
    Ok(run_id)
}

/// Mark a run cancelled — the drain skips its remaining tasks without dispatching them.
#[tauri::command]
pub fn cancel_run(state: State<'_, PipelineState>, run_id: String) -> Result<(), String> {
    state.cancelled.lock().map_err(|e| e.to_string())?.insert(run_id);
    Ok(())
}
