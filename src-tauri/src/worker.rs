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
use crate::pipeline::steps::{pump_once, start_discovery, EventSink};
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
    stage: String,
    status: String,
}

/// Emits live progress to the frontend via Tauri events.
struct TauriSink {
    app: AppHandle,
}
impl EventSink for TauriSink {
    fn step_done(&self, run_id: &str, stage: &str, status: &str) {
        let _ = self.app.emit(
            "run:step",
            StepEvent { run_id: run_id.into(), stage: stage.into(), status: status.into() },
        );
    }
    fn run_finished(&self, run_id: &str, status: &str) {
        let _ = self.app.emit(
            "run:finished",
            StepEvent { run_id: run_id.into(), stage: String::new(), status: status.into() },
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

    // Drain off the tokio reactor (blocking HTTP). The durable queue means a crash mid-run
    // leaves the remaining tasks persisted for a later drain to resume.
    let queue = state.queue.clone();
    let cancelled = state.cancelled.clone();
    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let scraper = ScrapingBeeScraper;
        let llm = OpenRouterLlm;
        let sink = TauriSink { app: app_for_thread };
        let is_cancelled = move |rid: &str| {
            cancelled.lock().map(|set| set.contains(rid)).unwrap_or(false)
        };
        loop {
            match pump_once(&*queue, &vault_path, &cfg, &scraper, &llm, &sink, &is_cancelled) {
                Ok(true) => continue,
                Ok(false) => break, // queue drained
                Err(e) => {
                    eprintln!("pipeline worker error: {e}");
                    break;
                }
            }
        }
    });

    Ok(run_id)
}

/// Mark a run cancelled — the drain skips its remaining tasks without dispatching them.
#[tauri::command]
pub fn cancel_run(state: State<'_, PipelineState>, run_id: String) -> Result<(), String> {
    state.cancelled.lock().map_err(|e| e.to_string())?.insert(run_id);
    Ok(())
}
