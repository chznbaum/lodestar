import { invoke } from "@tauri-apps/api/core";

// Types mirror the Rust structs in `src-tauri/src/check.rs` (serde snake_case;
// Rust `Option<T>` → `T | null`, numbers → `number`).

/** One step in a run's job-queue (a `stage`+`target` unit). */
export interface Step {
  stage: string;
  class: string;
  target: string;
  status: string;
  attempts: number;
  started_at: string | null;
  finished_at: string | null;
  error: string | null;
  cost: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
}

/**
 * Format cache activity for a step. Returns "" when both fields are null (the common case
 * — scrape/script steps and LLM steps that did not engage caching). When tokens are present,
 * returns a compact indicator showing read count (primary signal) and write count if present.
 *
 * Examples:
 *   null/null  → ""
 *   read=6656, write=null  → "· cache 6,656 read"
 *   read=6656, write=7000  → "· cache 6,656 read · 7,000 write"
 *   read=null, write=7000  → "· cache 7,000 write"   (cold-cache first call)
 */
export function formatCache(s: Pick<Step, "cache_read_tokens" | "cache_write_tokens">): string {
  if (s.cache_read_tokens == null && s.cache_write_tokens == null) return "";
  // "cache" labels the whole indicator once, so a write-only step (the cold-cache first call,
  // where only the prefix is written and nothing is read) still reads as cache activity.
  const parts: string[] = [];
  if (s.cache_read_tokens != null) {
    parts.push(`${s.cache_read_tokens.toLocaleString("en-US")} read`);
  }
  if (s.cache_write_tokens != null) {
    parts.push(`${s.cache_write_tokens.toLocaleString("en-US")} write`);
  }
  return "· cache " + parts.join(" · ");
}

/** Run-level rollup for the Checks run table (no steps). */
export interface CheckSummary {
  slug: string;
  kind: string;
  trigger: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  duration: string | null;
  /** The single entity this run is about (company slug for job_check, job slug for job_detail). */
  subject: string;
  roles_found: number;
  step_count: number;
  failed_count: number;
  credits: number;
  /** OpenRouter cost in micro-dollars (1_000_000 = $1.00). */
  usd_micro: number;
}

/** A full run, including its steps (the run-detail / step inspector). */
export interface Check {
  slug: string;
  kind: string;
  trigger: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  duration: string | null;
  /** The single entity this run is about (company slug for job_check, job slug for job_detail). */
  subject: string;
  roles_found: number;
  errors: number;
  steps: Step[];
}

/** List every run under `<vaultPath>/checks` as a summary, newest first. */
export function listChecks(vaultPath: string): Promise<CheckSummary[]> {
  return invoke<CheckSummary[]>("list_checks", { vaultPath });
}

/** Read one full run (with steps) by id. */
export function getCheck(vaultPath: string, id: string): Promise<Check> {
  return invoke<Check>("get_check", { vaultPath, id });
}
