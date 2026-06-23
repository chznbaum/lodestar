import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Stages belonging to the `job_detail` run (scrape + structure + gap work).
 * `research-gaps` is conditional — it is skipped (marked "skipped" in the strip)
 * when `gap-detect` finds no gaps and routes straight to `fit-score`.
 */
export const DETAIL_STAGES = [
  "jd-scrape",
  "structure-jd",
  "gap-detect",
  "research-gaps",
] as const;

/** Stages belonging to the `job_scoring` run (fit score + alignment). */
export const SCORING_STAGES = ["fit-score", "alignment"] as const;

/**
 * Full canonical ordered stage list across both runs (detail then scoring).
 * Kept for backwards-compat with any callers that reference it; prefer
 * `DETAIL_STAGES` / `SCORING_STAGES` for per-strip rendering.
 */
export const JOB_DETAIL_STAGES = [...DETAIL_STAGES, ...SCORING_STAGES] as const;

export type DetailStage = (typeof DETAIL_STAGES)[number];
export type ScoringStage = (typeof SCORING_STAGES)[number];
export type JobDetailStage = (typeof JOB_DETAIL_STAGES)[number];

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
  /** Job slug for job_detail/job_scoring runs; empty string for discovery runs. */
  subject: string;
  stage: string;
  status: string;
  detail?: string;
}

/** Start a job_scoring run for one slug. Rejects if a run is already in flight for that slug. */
export function rescoreJob(vaultPath: string, slug: string): Promise<string> {
  return invoke<string>("rescore_job", { vaultPath, slug });
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
