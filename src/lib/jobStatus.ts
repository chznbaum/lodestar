import { JOB_STATUSES } from "./job";

/** Human-readable labels for the six job statuses (lifecycle order). */
export const STATUS_LABELS: Record<(typeof JOB_STATUSES)[number], string> = {
  new: "New",
  detailed: "Detailed",
  scored: "Scored",
  selected: "Selected",
  applied: "Applied",
  skipped: "Skipped",
};

export type StatusDisplay =
  | { kind: "known"; status: string; label: string }
  | { kind: "anomaly"; raw: string | null; message: string };

/**
 * Classify a job's status value.
 * Known statuses → `{ kind: "known", status, label }`.
 * null → `{ kind: "anomaly", raw: null, message: "No status set" }`.
 * Unknown string → `{ kind: "anomaly", raw, message }` where the message names it unrecognized
 * and includes the raw value.
 */
export function classifyStatus(status: string | null): StatusDisplay {
  if (status === null) {
    return { kind: "anomaly", raw: null, message: "No status set" };
  }
  if (!isKnownStatus(status)) {
    return {
      kind: "anomaly",
      raw: status,
      message: `Unknown status: "${status}"`,
    };
  }
  return { kind: "known", status, label: STATUS_LABELS[status] };
}

/**
 * Type guard: true iff the status is non-null and a recognized `JOB_STATUSES` value.
 */
export function isKnownStatus(
  status: string | null,
): status is (typeof JOB_STATUSES)[number] {
  if (status === null) return false;
  return (JOB_STATUSES as readonly string[]).includes(status);
}

/**
 * Return the legal human-settable transition targets from the given current status.
 * Mirrors the backend `set_job_status` transition rules exactly:
 *   new      → [skipped]
 *   detailed → [skipped]
 *   scored   → [selected, skipped]
 *   selected → [applied]
 *   applied  → []  (terminal)
 *   skipped  → []  (terminal)
 *   null / unknown → []  (anomaly — frontend must show a warning, offer no transitions)
 */
export function nextHumanStatuses(current: string | null): string[] {
  switch (current) {
    case "new":
    case "detailed":
      return ["skipped"];
    case "scored":
      return ["selected", "skipped"];
    case "selected":
      return ["applied"];
    case "applied":
    case "skipped":
      return [];
    default:
      // null or unknown — anomaly
      return [];
  }
}
