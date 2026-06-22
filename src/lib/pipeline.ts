import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Mirrors the Rust `FetchJobDetailsOutcome` from `start_job_detail_runs`.
 * Each bucket carries slugs that were started, skipped (already done), or failed to start.
 */
export interface FetchJobDetailsOutcome {
  started: { slug: string; run_id: string }[];
  skipped: { slug: string; reason: string }[];
  failed: { slug: string; error: string }[];
}

/** Start a discovery run for a company; resolves to the run id (progress streams via events). */
export function fetchJobsForCompany(vaultPath: string, slug: string): Promise<string> {
  return invoke<string>("fetch_jobs_for_company", { vaultPath, slug });
}

/**
 * Start job-detail runs for the given slugs.
 * Returns a `FetchJobDetailsOutcome` describing which slugs were started, skipped, or failed to start.
 */
export function fetchJobDetails(
  vaultPath: string,
  slugs: string[],
): Promise<FetchJobDetailsOutcome> {
  return invoke<FetchJobDetailsOutcome>("fetch_job_details", { vaultPath, slugs });
}

/** Cancel an in-progress run; its remaining queued steps are dropped. */
export function cancelRun(runId: string): Promise<void> {
  return invoke<void>("cancel_run", { runId });
}

// Payloads mirror worker.rs's StepEvent.
export interface RunStepEvent {
  run_id: string;
  stage: string;
  status: string;
  detail?: string;
}

/** Subscribe to per-step progress. Returns an unlisten fn (call it on teardown). */
export function onRunStep(cb: (e: RunStepEvent) => void): Promise<UnlistenFn> {
  return listen<RunStepEvent>("run:step", (ev) => cb(ev.payload));
}

/** Subscribe to run completion (`status` = complete | failed | cancelled). */
export function onRunFinished(cb: (e: RunStepEvent) => void): Promise<UnlistenFn> {
  return listen<RunStepEvent>("run:finished", (ev) => cb(ev.payload));
}

/**
 * Human-readable label for a live pipeline phase.
 * Returns a non-empty phrase when `status === "running"` (the step has started but not finished);
 * returns `""` for completed/failed statuses (those are handled by the run-finished result line).
 * `detail` carries optional sub-phase info (e.g. `"stealth"` for the stealth-proxy retry).
 */
export function phaseLabel(stage: string, status: string, detail?: string): string {
  if (status === "running") {
    // Both careers-scrape and jd-scrape share the same scrape path and may retry via stealth.
    if ((stage === "careers-scrape" || stage === "jd-scrape") && detail === "stealth") {
      return "Retrying via stealth proxy…";
    }
    const m: Record<string, string> = {
      // Discovery stages
      "careers-scrape": "Scraping careers page…",
      "structure-listings": "Reading listings…",
      finalize: "Filtering to your titles…",
      // Job-detail stages
      "jd-scrape": "Fetching the JD…",
      "structure-jd": "Reading the JD…",
      "gap-detect": "Checking for gaps…",
      "research-gaps": "Researching gaps…",
      "fit-score": "Scoring fit…",
      alignment: "Writing the alignment…",
    };
    return m[stage] ?? "Working…";
  }
  return "";
}
