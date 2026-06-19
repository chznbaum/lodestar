import { describe, it, expect } from "vitest";
import { phaseLabel } from "./pipeline";

describe("phaseLabel", () => {
  it("returns the human phrase for careers-scrape running", () => {
    expect(phaseLabel("careers-scrape", "running")).toBe("Scraping careers page…");
  });
  it("returns the human phrase for structure-listings running", () => {
    expect(phaseLabel("structure-listings", "running")).toBe("Reading listings…");
  });
  it("returns the human phrase for finalize running", () => {
    expect(phaseLabel("finalize", "running")).toBe("Filtering to your titles…");
  });
  it("returns Working… for an unknown stage when running", () => {
    expect(phaseLabel("some-future-stage", "running")).toBe("Working…");
  });
  it("returns empty string for a completed step (non-running status)", () => {
    expect(phaseLabel("careers-scrape", "ok")).toBe("");
    expect(phaseLabel("finalize", "complete")).toBe("");
    expect(phaseLabel("careers-scrape", "failed")).toBe("");
  });
});
