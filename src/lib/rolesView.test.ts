import { describe, it, expect } from "vitest";
import { sortRoles, outcomeBySlug } from "./rolesView";
import type { Job } from "./job";
import type { FetchJobDetailsOutcome } from "./pipeline";

// Minimal Job factory — only fields used by sortRoles need to be real values.
const mk = (over: Partial<Job>): Job => ({
  slug: "x",
  title: "Role",
  company: null,
  url: null,
  level: null,
  location: null,
  comp_low: null,
  comp_high: null,
  comp_currency: null,
  comp_raw: null,
  comp_period: null,
  comp_equity: null,
  employment_type: null,
  yoe_min: null,
  yoe_max: null,
  tech_stack: [],
  required_skills: [],
  preferred_skills: [],
  reports_to: null,
  team: null,
  remote: null,
  location_constraints: null,
  visa_sponsorship: null,
  relocation: null,
  countries: [],
  metros: [],
  application_url: null,
  date_posted: null,
  last_seen: null,
  ats: null,
  fit_score: null,
  fit_seniority: null,
  fit_skills: null,
  fit_comp: null,
  fit_arrangement: null,
  fit_domain: null,
  researched: [],
  status: "new",
  skip_reason: null,
  jd_raw_file: null,
  jd_fetched: false,
  ...over,
});

describe("sortRoles", () => {
  it("places scored roles before unscored, scored by fit_score descending", () => {
    const jobs = [
      mk({ slug: "u-b", title: "B", fit_score: null }),
      mk({ slug: "s-40", title: "C", fit_score: 40 }),
      mk({ slug: "u-a", title: "A", fit_score: null }),
      mk({ slug: "s-80", title: "D", fit_score: 80 }),
    ];
    const result = sortRoles(jobs);
    expect(result.map((j) => j.slug)).toEqual(["s-80", "s-40", "u-a", "u-b"]);
  });

  it("breaks ties among equal fit_score by title ascending", () => {
    const jobs = [
      mk({ slug: "b", title: "Beta", fit_score: 80 }),
      mk({ slug: "a", title: "Alpha", fit_score: 80 }),
    ];
    const result = sortRoles(jobs);
    expect(result.map((j) => j.slug)).toEqual(["a", "b"]);
  });

  it("sorts unscored roles by title ascending", () => {
    const jobs = [
      mk({ slug: "c", title: "Charlie", fit_score: null }),
      mk({ slug: "a", title: "Alpha", fit_score: null }),
      mk({ slug: "b", title: "Beta", fit_score: null }),
    ];
    const result = sortRoles(jobs);
    expect(result.map((j) => j.slug)).toEqual(["a", "b", "c"]);
  });

  it("does not mutate the input array", () => {
    const jobs = [
      mk({ slug: "u", title: "U", fit_score: null }),
      mk({ slug: "s", title: "S", fit_score: 60 }),
    ];
    const original = [...jobs];
    sortRoles(jobs);
    expect(jobs.map((j) => j.slug)).toEqual(original.map((j) => j.slug));
  });

  it("handles a full mixed set with a tie (slug order: s-80a, s-80b, s-40, u-a, u-b)", () => {
    const jobs = [
      mk({ slug: "u-b", title: "Beta", fit_score: null }),
      mk({ slug: "s-40", title: "Middle", fit_score: 40 }),
      mk({ slug: "u-a", title: "Alpha", fit_score: null }),
      mk({ slug: "s-80b", title: "Zulu", fit_score: 80 }),
      mk({ slug: "s-80a", title: "Alpha", fit_score: 80 }),
    ];
    const result = sortRoles(jobs);
    expect(result.map((j) => j.slug)).toEqual(["s-80a", "s-80b", "s-40", "u-a", "u-b"]);
  });
});

describe("outcomeBySlug", () => {
  it("maps started slugs to { kind: 'started', runId }", () => {
    const outcome: FetchJobDetailsOutcome = {
      started: [{ slug: "role-a", run_id: "run-1" }],
      skipped: [],
      failed: [],
    };
    const map = outcomeBySlug(outcome);
    expect(map.get("role-a")).toEqual({ kind: "started", runId: "run-1" });
  });

  it("maps skipped slugs to { kind: 'skipped', detail }", () => {
    const outcome: FetchJobDetailsOutcome = {
      started: [],
      skipped: [{ slug: "role-b", reason: "already fetched" }],
      failed: [],
    };
    const map = outcomeBySlug(outcome);
    expect(map.get("role-b")).toEqual({ kind: "skipped", detail: "already fetched" });
  });

  it("maps failed slugs to { kind: 'failed', detail }", () => {
    const outcome: FetchJobDetailsOutcome = {
      started: [],
      skipped: [],
      failed: [{ slug: "role-c", error: "scrape timeout" }],
    };
    const map = outcomeBySlug(outcome);
    expect(map.get("role-c")).toEqual({ kind: "failed", detail: "scrape timeout" });
  });

  it("handles all three buckets in one outcome", () => {
    const outcome: FetchJobDetailsOutcome = {
      started: [{ slug: "s1", run_id: "r1" }],
      skipped: [{ slug: "sk1", reason: "already done" }],
      failed: [{ slug: "f1", error: "network error" }],
    };
    const map = outcomeBySlug(outcome);
    expect(map.get("s1")).toEqual({ kind: "started", runId: "r1" });
    expect(map.get("sk1")).toEqual({ kind: "skipped", detail: "already done" });
    expect(map.get("f1")).toEqual({ kind: "failed", detail: "network error" });
    expect(map.size).toBe(3);
  });
});
