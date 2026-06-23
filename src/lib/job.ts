import { invoke } from "@tauri-apps/api/core";

// Mirrors the Rust `Job` in src-tauri/src/job.rs (serde snake_case; `Option<T>` → `T | null`).
export interface Job {
  slug: string;
  title: string;
  company: string | null;
  url: string | null;
  level: string | null;
  location: string | null;
  // Comp fields
  comp_low: number | null;
  comp_high: number | null;
  comp_currency: string | null;
  comp_raw: string | null;
  comp_period: string | null;
  comp_equity: string | null;
  // Role classification
  employment_type: string | null;
  yoe_min: number | null;
  yoe_max: number | null;
  tech_stack: string[];
  required_skills: string[];
  preferred_skills: string[];
  // Org context
  reports_to: string | null;
  team: string | null;
  // Location / logistics
  remote: string | null;
  location_constraints: string | null;
  visa_sponsorship: string | null;
  relocation: string | null;
  countries: string[];
  metros: string[];
  application_url: string | null;
  // Pipeline metadata
  date_posted: string | null;
  last_seen: string | null;
  ats: string | null;
  fit_score: number | null;
  fit_seniority: number | null;
  fit_skills: number | null;
  fit_comp: number | null;
  fit_arrangement: number | null;
  fit_domain: number | null;
  /** Fields populated by the research-gaps stage (provenance). */
  researched: string[];
  /** new | detailed | scored | selected | applied | skipped */
  status: string | null;
  jd_raw_file: string | null;
  /** Derived: a structured JD has been fetched (powers the gate's new-vs-already-fetched). */
  jd_fetched: boolean;
}

/** The full job record plus its markdown body sections (returned by `get_job`). */
export interface JobDetail extends Job {
  body: string;
}

/** Valid job status values — mirrors the Rust `JOB_STATUSES` constant verbatim (lifecycle order). */
export const JOB_STATUSES = ["new", "detailed", "scored", "selected", "applied", "skipped"] as const;

/** Statuses the human may set via `set_job_status` (the decision control). */
export const HUMAN_SETTABLE_STATUSES = ["selected", "applied", "skipped"] as const;

/** Read + parse every job note under `<vaultPath>/jobs`. */
export function listJobs(vaultPath: string): Promise<Job[]> {
  return invoke<Job[]>("list_jobs", { vaultPath });
}

/** Read a single job note by slug, returning its typed fields plus the raw body. */
export function getJob(vaultPath: string, slug: string): Promise<JobDetail> {
  return invoke<JobDetail>("get_job", { vaultPath, slug });
}

/** Set the job's status (validated on the backend). */
export function setJobStatus(vaultPath: string, slug: string, status: string): Promise<void> {
  return invoke("set_job_status", { vaultPath, slug, status });
}

/** Write a single scalar field on the job note. */
export function updateJobField(
  vaultPath: string,
  slug: string,
  field: string,
  value: string,
): Promise<void> {
  return invoke("update_job_field", { vaultPath, slug, field, value });
}

/** Write a list field on the job note. Mirrors `set_job_list_field` (backend). */
export function setJobListField(
  vaultPath: string,
  slug: string,
  field: string,
  values: string[],
): Promise<void> {
  return invoke("set_job_list_field", { vaultPath, slug, field, values });
}

// ---------------------------------------------------------------------------
// Enum value constants for inline-edit selects.
// Mirrors src-tauri/src/job.rs constants verbatim (EMPLOYMENT_TYPES, COMP_PERIODS,
// REMOTE_KINDS, SPONSORSHIP, VALID_LEVELS).
// ---------------------------------------------------------------------------

/** mirrors src-tauri/src/job.rs EMPLOYMENT_TYPES */
export const EMPLOYMENT_TYPES = [
  "full_time",
  "part_time",
  "contract",
  "fractional",
  "internship",
  "temporary",
] as const;

/** mirrors src-tauri/src/job.rs COMP_PERIODS */
export const COMP_PERIODS = [
  "annual",
  "hourly",
  "daily",
  "monthly",
  "weekly",
  "biweekly",
] as const;

/** mirrors src-tauri/src/job.rs REMOTE_KINDS */
export const REMOTE_KINDS = ["remote", "hybrid", "onsite"] as const;

/** mirrors src-tauri/src/job.rs SPONSORSHIP (used for visa_sponsorship + relocation) */
export const SPONSORSHIP = ["offered", "not_offered", "unspecified"] as const;
