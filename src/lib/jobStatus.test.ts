import { describe, it, expect } from "vitest";
import { STATUS_LABELS, classifyStatus, isKnownStatus } from "./jobStatus";
import { JOB_STATUSES } from "./job";

describe("STATUS_LABELS", () => {
  it("covers all six JOB_STATUSES with correct labels", () => {
    expect(STATUS_LABELS["new"]).toBe("New");
    expect(STATUS_LABELS["detailed"]).toBe("Detailed");
    expect(STATUS_LABELS["scored"]).toBe("Scored");
    expect(STATUS_LABELS["selected"]).toBe("Selected");
    expect(STATUS_LABELS["applied"]).toBe("Applied");
    expect(STATUS_LABELS["skipped"]).toBe("Skipped");
  });

  it("has exactly the JOB_STATUSES keys (no drift)", () => {
    expect(Object.keys(STATUS_LABELS).sort()).toEqual([...JOB_STATUSES].sort());
  });
});

describe("classifyStatus — known statuses", () => {
  it("new → kind known, status 'new', label 'New'", () => {
    const r = classifyStatus("new");
    expect(r).toEqual({ kind: "known", status: "new", label: "New" });
  });

  it("detailed → kind known, label 'Detailed'", () => {
    const r = classifyStatus("detailed");
    expect(r).toEqual({ kind: "known", status: "detailed", label: "Detailed" });
  });

  it("scored → kind known, label 'Scored'", () => {
    const r = classifyStatus("scored");
    expect(r).toEqual({ kind: "known", status: "scored", label: "Scored" });
  });

  it("selected → kind known, label 'Selected'", () => {
    const r = classifyStatus("selected");
    expect(r).toEqual({ kind: "known", status: "selected", label: "Selected" });
  });

  it("applied → kind known, label 'Applied'", () => {
    const r = classifyStatus("applied");
    expect(r).toEqual({ kind: "known", status: "applied", label: "Applied" });
  });

  it("skipped → kind known, label 'Skipped'", () => {
    const r = classifyStatus("skipped");
    expect(r).toEqual({ kind: "known", status: "skipped", label: "Skipped" });
  });
});

describe("classifyStatus — anomalies", () => {
  it("null → anomaly with message 'No status set'", () => {
    const r = classifyStatus(null);
    expect(r.kind).toBe("anomaly");
    if (r.kind === "anomaly") {
      expect(r.raw).toBeNull();
      expect(r.message).toBe("No status set");
    }
  });

  it("unknown string → anomaly whose message names it unknown and includes the raw value", () => {
    const r = classifyStatus("garbage");
    expect(r.kind).toBe("anomaly");
    if (r.kind === "anomaly") {
      expect(r.raw).toBe("garbage");
      expect(r.message).toContain("garbage");
      // must clearly indicate it is unknown/unrecognized
      expect(r.message.toLowerCase()).toMatch(/unknown|unrecognized/);
    }
  });
});

describe("isKnownStatus", () => {
  it("returns true for a known status", () => {
    expect(isKnownStatus("scored")).toBe(true);
    expect(isKnownStatus("new")).toBe(true);
  });

  it("returns false for an unknown string", () => {
    expect(isKnownStatus("garbage")).toBe(false);
  });

  it("returns false for null", () => {
    expect(isKnownStatus(null)).toBe(false);
  });
});
