import { describe, it, expect } from "vitest";
import { segments } from "./highlight";

describe("segments", () => {
  it("returns a single unmarked run for an empty or whitespace query", () => {
    expect(segments("Financial Services", "")).toEqual([{ text: "Financial Services", mark: false }]);
    expect(segments("Financial Services", "   ")).toEqual([{ text: "Financial Services", mark: false }]);
  });

  it("marks a prefix match", () => {
    expect(segments("Financial", "fin")).toEqual([
      { text: "Fin", mark: true },
      { text: "ancial", mark: false },
    ]);
  });

  it("marks a mid-string match with both flanks", () => {
    expect(segments("Crowdfinancing", "fin")).toEqual([
      { text: "Crowd", mark: false },
      { text: "fin", mark: true },
      { text: "ancing", mark: false },
    ]);
  });

  it("marks a suffix match with no trailing run", () => {
    expect(segments("Vega Sec", "sec")).toEqual([
      { text: "Vega ", mark: false },
      { text: "Sec", mark: true },
    ]);
  });

  it("is case-insensitive but preserves the original casing of text", () => {
    expect(segments("SECURITY", "sec")).toEqual([
      { text: "SEC", mark: true },
      { text: "URITY", mark: false },
    ]);
  });

  it("marks only the first occurrence", () => {
    expect(segments("aXaXa", "x")).toEqual([
      { text: "a", mark: false },
      { text: "X", mark: true },
      { text: "aXa", mark: false },
    ]);
  });

  it("returns the text unmarked when there is no match", () => {
    expect(segments("Healthcare", "zzz")).toEqual([{ text: "Healthcare", mark: false }]);
  });
});
