import { describe, it, expect } from "vitest";
import { humanize } from "./labels";

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
