import { describe, it, expect } from "vitest";
import { humanize, monogram, relativeDate } from "./labels";

describe("humanize", () => {
  it("title-cases snake_case", () => {
    expect(humanize("financial_services")).toBe("Financial Services");
    expect(humanize("human_resources")).toBe("Human Resources");
    expect(humanize("scaleup")).toBe("Scaleup");
  });
  it("applies overrides for special forms", () => {
    expect(humanize("series_c_plus")).toBe("Series C+");
    expect(humanize("pre_seed")).toBe("Pre-seed");
    expect(humanize("mid_market")).toBe("Mid-market");
    expect(humanize("remote_first")).toBe("Remote-first");
    expect(humanize("fully_remote")).toBe("Fully remote");
    expect(humanize("b2b")).toBe("B2B");
    expect(humanize("ai")).toBe("AI");
  });
  it("passes through empty", () => {
    expect(humanize("")).toBe("");
  });
});

describe("relativeDate", () => {
  it("returns 'Xd ago' for multi-day differences", () => {
    expect(relativeDate("2026-06-10", "2026-06-15")).toBe("5d ago");
  });
  it("returns 'yesterday' for 1-day difference", () => {
    expect(relativeDate("2026-06-14", "2026-06-15")).toBe("yesterday");
  });
  it("returns 'today' for same day", () => {
    expect(relativeDate("2026-06-15", "2026-06-15")).toBe("today");
  });
});

describe("monogram", () => {
  it("takes the first two characters, preserving case", () => {
    expect(monogram("Stripe")).toBe("St");
    expect(monogram("15Five")).toBe("15");
    expect(monogram("1Password")).toBe("1P");
    expect(monogram("  Acme")).toBe("Ac");
    expect(monogram("")).toBe("?");
  });
});
