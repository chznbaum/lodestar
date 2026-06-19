import { describe, it, expect } from "vitest";
import { levelLabel, LEVEL_LABELS } from "./level";

describe("levelLabel", () => {
  it("returns the human label for a valid level", () => {
    expect(levelLabel("vp")).toBe("VP");
    expect(levelLabel("junior")).toBe("Junior");
    expect(levelLabel("senior")).toBe("Senior");
    expect(levelLabel("front-line-mgmt")).toBe("Front-line management");
    expect(levelLabel("middle-mgmt")).toBe("Middle management");
    expect(levelLabel("dept-head")).toBe("Department head");
    expect(levelLabel("mid")).toBe("Mid-level");
    expect(levelLabel("c-suite")).toBe("C-suite");
  });

  it("returns empty string for an unrecognized value", () => {
    expect(levelLabel("wizard")).toBe("");
    expect(levelLabel("senior-ic")).toBe("");
    expect(levelLabel("head-of-eng")).toBe("");
  });

  it("returns empty string for null", () => {
    expect(levelLabel(null)).toBe("");
  });

  it("returns empty string for undefined", () => {
    expect(levelLabel(undefined)).toBe("");
  });

  it("LEVEL_LABELS has exactly 8 entries matching VALID_LEVELS", () => {
    const keys = Object.keys(LEVEL_LABELS);
    expect(keys).toHaveLength(8);
    expect(keys).toContain("junior");
    expect(keys).toContain("mid");
    expect(keys).toContain("senior");
    expect(keys).toContain("front-line-mgmt");
    expect(keys).toContain("middle-mgmt");
    expect(keys).toContain("dept-head");
    expect(keys).toContain("vp");
    expect(keys).toContain("c-suite");
  });
});
