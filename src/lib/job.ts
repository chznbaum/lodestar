import { invoke } from "@tauri-apps/api/core";

// Mirrors the Rust `Job` in src-tauri/src/job.rs (serde snake_case; `Option<T>` → `T | null`).
export interface Job {
  slug: string;
  title: string;
  company: string | null;
  url: string | null;
  level: string | null;
  location: string | null;
  comp_low: number | null;
  comp_high: number | null;
  comp_currency: string | null;
  comp_raw: string | null;
  date_posted: string | null;
  last_seen: string | null;
  ats: string | null;
  tech_stack: string[];
  fit_score: number | null;
  status: string | null;
  skip_reason: string | null;
  jd_raw_file: string | null;
  /** Derived: a structured JD has been fetched (powers the gate's new-vs-already-fetched). */
  jd_fetched: boolean;
}

/** Read + parse every job note under `<vaultPath>/jobs`. */
export function listJobs(vaultPath: string): Promise<Job[]> {
  return invoke<Job[]>("list_jobs", { vaultPath });
}
