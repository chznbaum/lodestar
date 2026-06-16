import { describe, it, expect } from "vitest";
import { filterOptions, type ComboOption } from "./combobox";

const OPTIONS: ComboOption[] = [
  { label: "Financial Services", value: "financial_services", aliases: ["fintech"] },
  { label: "Insurance", value: "insurance", aliases: ["fintech-adjacent"] },
  { label: "Personal Finance", value: "personal_finance" },
  { label: "Crowdfinancing", value: "crowdfinancing" },
  { label: "Healthcare", value: "healthcare" },
];

describe("filterOptions", () => {
  it("returns all options unchanged for an empty query", () => {
    expect(filterOptions(OPTIONS, "")).toEqual(OPTIONS);
    // whitespace-only queries are also empty
    expect(filterOptions(OPTIONS, "   ")).toEqual(OPTIONS);
  });

  it("matches a label substring (case-insensitive)", () => {
    const result = filterOptions(OPTIONS, "finance");
    expect(result.map((o) => o.value)).toContain("personal_finance");
    expect(result.map((o) => o.value)).not.toContain("healthcare");
  });

  it("matches mid-word, not just prefix", () => {
    const result = filterOptions(OPTIONS, "fin");
    expect(result.map((o) => o.value)).toContain("crowdfinancing");
  });

  it("matches via an alias when the label does not contain the query", () => {
    // "fintech" is an alias of "Financial Services" whose label lacks "fintech"
    const result = filterOptions(OPTIONS, "fintech");
    expect(result.map((o) => o.value)).toContain("financial_services");
    // "fintech-adjacent" alias also matches Insurance
    expect(result.map((o) => o.value)).toContain("insurance");
  });

  it("does NOT match on the raw slug", () => {
    // "_services" only exists in the slug, not the label or aliases
    expect(filterOptions(OPTIONS, "_services")).toEqual([]);
    // underscore form of personal_finance is slug-only
    expect(filterOptions(OPTIONS, "personal_finance")).toEqual([]);
  });

  it("returns [] when nothing matches", () => {
    expect(filterOptions(OPTIONS, "zzz")).toEqual([]);
  });

  it("ranks a label-prefix match before an alias-only match", () => {
    // "fin" → Financial Services (label prefix) must sort before Insurance (alias only)
    const result = filterOptions(OPTIONS, "fin");
    const fsIdx = result.findIndex((o) => o.value === "financial_services");
    const insIdx = result.findIndex((o) => o.value === "insurance");
    expect(fsIdx).toBeGreaterThanOrEqual(0);
    expect(insIdx).toBeGreaterThan(fsIdx);
  });

  it("ranks label-substring above alias-only, and prefix above substring", () => {
    const result = filterOptions(OPTIONS, "fin").map((o) => o.value);
    // Financial Services = label-prefix (tier 0)
    // Personal Finance & Crowdfinancing = label-substring (tier 1)
    // Insurance = alias-only (tier 2)
    expect(result.indexOf("financial_services")).toBeLessThan(result.indexOf("personal_finance"));
    expect(result.indexOf("personal_finance")).toBeLessThan(result.indexOf("insurance"));
  });

  it("is stable within a tier (preserves original order)", () => {
    // Personal Finance precedes Crowdfinancing in the source array; both are
    // label-substring matches for "fin", so order is preserved.
    const result = filterOptions(OPTIONS, "fin").map((o) => o.value);
    expect(result.indexOf("personal_finance")).toBeLessThan(result.indexOf("crowdfinancing"));
  });
});
