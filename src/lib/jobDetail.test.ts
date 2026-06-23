import { describe, it, expect } from "vitest";
import { extractSection, jobSections, subScoreRows, hasDealbreaker } from "./jobDetail";
import type { Job } from "./job";

// ---------------------------------------------------------------------------
// extractSection
// ---------------------------------------------------------------------------

describe("extractSection — heading present mid-body", () => {
  it("returns the trimmed section body, stopping before the next ## heading", () => {
    const body = [
      "## First section",
      "Line A",
      "Line B",
      "",
      "## Second section",
      "Line C",
    ].join("\n");
    expect(extractSection(body, "## First section")).toBe("Line A\nLine B");
  });
});

describe("extractSection — section at EOF", () => {
  it("returns content to end of body when no following ## heading", () => {
    const body = [
      "## Only section",
      "Some content here.",
      "",
      "More content.",
    ].join("\n");
    expect(extractSection(body, "## Only section")).toBe(
      "Some content here.\n\nMore content.",
    );
  });
});

describe("extractSection — ### subheading does NOT end the section", () => {
  it("includes lines after a ### subheading and only stops at the next ##", () => {
    const body = [
      "## Parent section",
      "Intro line.",
      "### Sub-section",
      "Sub content.",
      "## Next section",
      "Out of scope.",
    ].join("\n");
    expect(extractSection(body, "## Parent section")).toBe(
      "Intro line.\n### Sub-section\nSub content.",
    );
  });
});

describe("extractSection — heading absent", () => {
  it("returns null when the heading is not found", () => {
    const body = "## Some other section\nContent here.";
    expect(extractSection(body, "## Missing section")).toBeNull();
  });
});

describe("extractSection — heading match tolerates surrounding whitespace", () => {
  it("matches a heading line with leading/trailing whitespace via .trim()", () => {
    const body = [
      "  ## Whitespace section  ",
      "Indented content.",
      "## Next",
      "Out.",
    ].join("\n");
    expect(extractSection(body, "## Whitespace section")).toBe(
      "Indented content.",
    );
  });
});

// ---------------------------------------------------------------------------
// jobSections
// ---------------------------------------------------------------------------

const FULL_BODY = [
  "## Alignment analysis",
  "Strong fit — worth pursuing.",
  "",
  "## Fit flags",
  "- **comp_floor** [DEALBREAKER]: comp is below floor.",
  "",
  "## Research notes",
  "Researched comp_low via levels.fyi.",
  "",
  "## JD — structured",
  "**Title:** Senior Engineer",
].join("\n");

describe("jobSections — body containing all four sections", () => {
  it("extracts alignment correctly", () => {
    const s = jobSections(FULL_BODY);
    expect(s.alignment).toBe("Strong fit — worth pursuing.");
  });

  it("extracts fitFlags correctly", () => {
    const s = jobSections(FULL_BODY);
    expect(s.fitFlags).toBe("- **comp_floor** [DEALBREAKER]: comp is below floor.");
  });

  it("extracts research correctly", () => {
    const s = jobSections(FULL_BODY);
    expect(s.research).toBe("Researched comp_low via levels.fyi.");
  });

  it("extracts jdStructured using the em-dash heading (U+2014)", () => {
    const s = jobSections(FULL_BODY);
    expect(s.jdStructured).toBe("**Title:** Senior Engineer");
  });
});

describe("jobSections — body missing some sections", () => {
  it("returns null for absent sections", () => {
    const body = [
      "## Alignment analysis",
      "Some narrative.",
    ].join("\n");
    const s = jobSections(body);
    expect(s.alignment).toBe("Some narrative.");
    expect(s.fitFlags).toBeNull();
    expect(s.research).toBeNull();
    expect(s.jdStructured).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// subScoreRows
// ---------------------------------------------------------------------------

function makeJob(overrides: Partial<Job> = {}): Job {
  return {
    slug: "test-job",
    title: "Test Job",
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
    jd_raw_file: null,
    jd_fetched: false,
    ...overrides,
  };
}

describe("subScoreRows — job with all five sub-scores set", () => {
  const job = makeJob({
    fit_seniority: 100,
    fit_skills: 80,
    fit_comp: 60,
    fit_arrangement: 40,
    fit_domain: 20,
  });

  it("returns exactly five rows", () => {
    expect(subScoreRows(job)).toHaveLength(5);
  });

  it("first row is seniority with correct key/label/value", () => {
    const rows = subScoreRows(job);
    expect(rows[0]).toEqual({ key: "seniority", label: "Seniority", value: 100 });
  });

  it("second row is skills with correct key/label/value", () => {
    const rows = subScoreRows(job);
    expect(rows[1]).toEqual({ key: "skills", label: "Skills", value: 80 });
  });

  it("third row is comp with correct key/label/value", () => {
    const rows = subScoreRows(job);
    expect(rows[2]).toEqual({ key: "comp", label: "Comp", value: 60 });
  });

  it("fourth row is arrangement with correct key/label/value", () => {
    const rows = subScoreRows(job);
    expect(rows[3]).toEqual({ key: "arrangement", label: "Arrangement", value: 40 });
  });

  it("fifth row is domain with correct key/label/value", () => {
    const rows = subScoreRows(job);
    expect(rows[4]).toEqual({ key: "domain", label: "Domain", value: 20 });
  });
});

describe("subScoreRows — unscored job (all five null)", () => {
  const job = makeJob();

  it("returns exactly five rows", () => {
    expect(subScoreRows(job)).toHaveLength(5);
  });

  it("all rows have value: null", () => {
    const rows = subScoreRows(job);
    expect(rows.every((r) => r.value === null)).toBe(true);
  });

  it("keys are in correct order", () => {
    const rows = subScoreRows(job);
    expect(rows.map((r) => r.key)).toEqual([
      "seniority",
      "skills",
      "comp",
      "arrangement",
      "domain",
    ]);
  });
});

// ---------------------------------------------------------------------------
// hasDealbreaker
// ---------------------------------------------------------------------------

describe("hasDealbreaker", () => {
  it("returns true when fitFlags contains [DEALBREAKER]", () => {
    expect(
      hasDealbreaker("- **comp_floor** [DEALBREAKER]: comp is below floor."),
    ).toBe(true);
  });

  it("returns false when fitFlags contains only [CAUTION]", () => {
    expect(
      hasDealbreaker("- **remote** [CAUTION]: arrangement is uncertain."),
    ).toBe(false);
  });

  it("returns false when fitFlags is null", () => {
    expect(hasDealbreaker(null)).toBe(false);
  });
});
