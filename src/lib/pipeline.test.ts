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
  it("returns stealth label when careers-scrape running with detail=stealth", () => {
    expect(phaseLabel("careers-scrape", "running", "stealth")).toBe("Retrying via stealth proxy…");
  });
  it("returns normal scrape label when careers-scrape running with no detail", () => {
    expect(phaseLabel("careers-scrape", "running", undefined)).toBe("Scraping careers page…");
  });

  // Job-detail stages
  it("returns 'Fetching the JD…' for jd-scrape running", () => {
    expect(phaseLabel("jd-scrape", "running")).toBe("Fetching the JD…");
  });
  it("returns 'Reading the JD…' for structure-jd running", () => {
    expect(phaseLabel("structure-jd", "running")).toBe("Reading the JD…");
  });
  it("returns 'Checking for gaps…' for gap-detect running", () => {
    expect(phaseLabel("gap-detect", "running")).toBe("Checking for gaps…");
  });
  it("returns 'Researching gaps…' for research-gaps running", () => {
    expect(phaseLabel("research-gaps", "running")).toBe("Researching gaps…");
  });
  it("returns 'Scoring fit…' for fit-score running", () => {
    expect(phaseLabel("fit-score", "running")).toBe("Scoring fit…");
  });
  it("returns 'Writing the alignment…' for alignment running", () => {
    expect(phaseLabel("alignment", "running")).toBe("Writing the alignment…");
  });

  // Stealth generalization: jd-scrape also triggers the stealth label
  it("returns stealth label when jd-scrape running with detail=stealth", () => {
    expect(phaseLabel("jd-scrape", "running", "stealth")).toBe("Retrying via stealth proxy…");
  });
  it("returns normal jd-scrape label when jd-scrape running with no detail", () => {
    expect(phaseLabel("jd-scrape", "running", undefined)).toBe("Fetching the JD…");
  });
});
