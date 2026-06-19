/** Profession-agnostic seniority level labels. Machine values must stay in sync with
 * `VALID_LEVELS` in `src-tauri/src/job.rs` and the `level` enum in the LLM prompt. */
export const LEVEL_LABELS: Record<string, string> = {
  junior: "Junior",
  mid: "Mid-level",
  senior: "Senior",
  "front-line-mgmt": "Front-line management",
  "middle-mgmt": "Middle management",
  "dept-head": "Department head",
  vp: "VP",
  "c-suite": "C-suite",
};

/** Returns the human label for a level value, or `""` for unknown/null/undefined. */
export function levelLabel(v?: string | null): string {
  return v ? (LEVEL_LABELS[v] ?? "") : "";
}
