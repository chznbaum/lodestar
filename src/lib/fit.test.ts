import { describe, it, expect } from "vitest";
import { fitBand } from "./fit";

describe("fitBand", () => {
  it("scores 0 → mismatch", () => {
    const r = fitBand(0);
    expect(r.key).toBe("mismatch");
    expect(r.label).toBe("Mismatch");
  });

  it("scores 19 → mismatch", () => {
    const r = fitBand(19);
    expect(r.key).toBe("mismatch");
    expect(r.label).toBe("Mismatch");
  });

  it("scores 20 → weak", () => {
    const r = fitBand(20);
    expect(r.key).toBe("weak");
    expect(r.label).toBe("Weak");
  });

  it("scores 39 → weak", () => {
    const r = fitBand(39);
    expect(r.key).toBe("weak");
    expect(r.label).toBe("Weak");
  });

  it("scores 40 → partial", () => {
    const r = fitBand(40);
    expect(r.key).toBe("partial");
    expect(r.label).toBe("Partial");
  });

  it("scores 59 → partial", () => {
    const r = fitBand(59);
    expect(r.key).toBe("partial");
    expect(r.label).toBe("Partial");
  });

  it("scores 60 → good", () => {
    const r = fitBand(60);
    expect(r.key).toBe("good");
    expect(r.label).toBe("Good");
  });

  it("scores 79 → good", () => {
    const r = fitBand(79);
    expect(r.key).toBe("good");
    expect(r.label).toBe("Good");
  });

  it("scores 80 → strong", () => {
    const r = fitBand(80);
    expect(r.key).toBe("strong");
    expect(r.label).toBe("Strong");
  });

  it("scores 100 → strong", () => {
    const r = fitBand(100);
    expect(r.key).toBe("strong");
    expect(r.label).toBe("Strong");
  });

  it("null → unscored", () => {
    const r = fitBand(null);
    expect(r.key).toBe("unscored");
    expect(r.label).toBe("Unscored");
  });
});
