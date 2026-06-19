import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Start a discovery run for a company; resolves to the run id (progress streams via events). */
export function fetchJobsForCompany(vaultPath: string, slug: string): Promise<string> {
  return invoke<string>("fetch_jobs_for_company", { vaultPath, slug });
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
}

/** Subscribe to per-step progress. Returns an unlisten fn (call it on teardown). */
export function onRunStep(cb: (e: RunStepEvent) => void): Promise<UnlistenFn> {
  return listen<RunStepEvent>("run:step", (ev) => cb(ev.payload));
}

/** Subscribe to run completion (`status` = complete | failed | cancelled). */
export function onRunFinished(cb: (e: RunStepEvent) => void): Promise<UnlistenFn> {
  return listen<RunStepEvent>("run:finished", (ev) => cb(ev.payload));
}
