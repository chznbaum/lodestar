import type { Job } from "./job";
import type { FetchJobDetailsOutcome } from "./pipeline";

/**
 * Sort roles for the Triage List:
 * - Scored roles (fit_score != null) first, ordered by fit_score descending.
 * - Ties among equal fit_score ordered by title ascending.
 * - Unscored roles (fit_score == null) after, ordered by title ascending.
 * Does not mutate the input array.
 */
export function sortRoles(jobs: Job[]): Job[] {
  return [...jobs].sort((a, b) => {
    const aScored = a.fit_score !== null;
    const bScored = b.fit_score !== null;

    // Scored before unscored
    if (aScored && !bScored) return -1;
    if (!aScored && bScored) return 1;

    if (aScored && bScored) {
      // Both scored: descending by fit_score, then ascending by title
      const scoreDiff = (b.fit_score as number) - (a.fit_score as number);
      if (scoreDiff !== 0) return scoreDiff;
      return a.title.localeCompare(b.title);
    }

    // Both unscored: ascending by title
    return a.title.localeCompare(b.title);
  });
}

/** Per-slug outcome entry from a `FetchJobDetailsOutcome`. */
export type SlugOutcome =
  | { kind: "started"; runId: string }
  | { kind: "skipped"; detail: string }
  | { kind: "failed"; detail: string };

/**
 * Build a Map from slug → outcome entry, mapping the Rust snake_case fields
 * (`run_id` → `runId`; `reason`/`error` → `detail`).
 */
export function outcomeBySlug(o: FetchJobDetailsOutcome): Map<string, SlugOutcome> {
  const map = new Map<string, SlugOutcome>();
  for (const { slug, run_id } of o.started) {
    map.set(slug, { kind: "started", runId: run_id });
  }
  for (const { slug, reason } of o.skipped) {
    map.set(slug, { kind: "skipped", detail: reason });
  }
  for (const { slug, error } of o.failed) {
    map.set(slug, { kind: "failed", detail: error });
  }
  return map;
}
