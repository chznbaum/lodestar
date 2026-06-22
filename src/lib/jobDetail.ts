import type { Job } from "./job";

/**
 * Extract the body of a `## ` section from an already-frontmatter-stripped body string.
 *
 * - Finds the line whose `.trim()` equals `heading.trim()`.
 * - Collects lines after it, stopping at the next line that starts with `"## "`.
 *   A `### ` subheading does NOT stop collection (mirrors the Rust `extract_section` rule).
 * - Returns the collected lines joined with `"\n"`, trimmed.
 * - Returns `null` if the heading line is not found.
 *
 * @param body    The note body text (frontmatter already stripped — `JobDetail.body`).
 * @param heading The full heading string including the `## ` prefix, e.g. `"## Alignment analysis"`.
 */
export function extractSection(body: string, heading: string): string | null {
  const target = heading.trim();
  const lines = body.split("\n");
  let headingIdx = -1;

  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === target) {
      headingIdx = i;
      break;
    }
  }

  if (headingIdx === -1) {
    return null;
  }

  const collected: string[] = [];
  for (let i = headingIdx + 1; i < lines.length; i++) {
    if (lines[i].startsWith("## ")) {
      break;
    }
    collected.push(lines[i]);
  }

  return collected.join("\n").trim();
}

/**
 * Extract the four named body sections from a job's body text.
 *
 * Heading strings are copied verbatim from the Rust `set_job_section` calls in
 * `src-tauri/src/pipeline/steps.rs`. The `jdStructured` heading uses an em-dash (U+2014).
 */
export function jobSections(body: string): {
  alignment: string | null;
  fitFlags: string | null;
  research: string | null;
  jdStructured: string | null;
} {
  return {
    alignment: extractSection(body, "## Alignment analysis"),
    fitFlags: extractSection(body, "## Fit flags"),
    research: extractSection(body, "## Research notes"),
    jdStructured: extractSection(body, "## JD — structured"),
  };
}

/**
 * Return the five fit sub-score rows in a stable display order.
 * Each row: `{ key, label, value }` where `value` is `null` for an unscored job.
 */
export function subScoreRows(
  job: Job,
): { key: string; label: string; value: number | null }[] {
  return [
    { key: "seniority", label: "Seniority", value: job.fit_seniority },
    { key: "skills", label: "Skills", value: job.fit_skills },
    { key: "comp", label: "Comp", value: job.fit_comp },
    { key: "arrangement", label: "Arrangement", value: job.fit_arrangement },
    { key: "domain", label: "Domain", value: job.fit_domain },
  ];
}

/**
 * Returns `true` iff `fitFlags` is non-null and contains the literal marker `"[DEALBREAKER]"`.
 * A flags section with only `[CAUTION]` returns `false`.
 */
export function hasDealbreaker(fitFlags: string | null): boolean {
  return fitFlags !== null && fitFlags.includes("[DEALBREAKER]");
}
