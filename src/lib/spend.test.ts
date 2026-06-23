import { describe, it, expect } from "vitest";
import { runSpend, totalSpend } from "./spend";
import type { Step, Check } from "./check";

const step = (cls: string, cost: number | null): Step => ({
  stage: "s",
  class: cls,
  target: "t",
  status: "ok",
  attempts: 1,
  started_at: null,
  finished_at: null,
  error: null,
  cost,
});

describe("runSpend", () => {
  it("sums scrape costs as credits and llm costs as micro-dollars", () => {
    // 25 credits; $0.02 + $0.10 expressed in micro-dollars
    const s = runSpend([
      step("scrape", 25),
      step("script", null),
      step("llm", 20_000),
      step("llm+web", 100_000),
    ]);
    expect(s.credits).toBe(25);
    expect(s.usdMicro).toBe(120_000); // exact integer sum = $0.12
  });

  it("ignores null costs and script steps", () => {
    expect(runSpend([step("script", null), step("scrape", null)])).toEqual({ credits: 0, usdMicro: 0 });
  });
});

describe("totalSpend", () => {
  it("accumulates across runs", () => {
    const run = (steps: Step[]): Check => ({
      slug: "r",
      kind: "job_check",
      trigger: "manual",
      status: "complete",
      started_at: null,
      finished_at: null,
      duration: null,
      subject: "",
      roles_found: 0,
      errors: 0,
      steps,
    });
    const total = totalSpend([run([step("scrape", 25)]), run([step("scrape", 25), step("llm", 50_000)])]);
    expect(total.credits).toBe(50);
    expect(total.usdMicro).toBe(50_000); // $0.05
  });
});
