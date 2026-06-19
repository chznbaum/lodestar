import type { Step, Check } from "$lib/check";

/** Spend tallied from recorded step costs: ScrapingBee credits + OpenRouter micro-dollars
 *  (1_000_000 = $1.00). Both integers, so the sums stay exact (well under 2^53 in JS). */
export interface Spend {
  credits: number;
  usdMicro: number;
}

/** A run's spend — the unit of each step's `cost` is implied by its `class`. */
export function runSpend(steps: Step[]): Spend {
  let credits = 0;
  let usdMicro = 0;
  for (const s of steps) {
    if (s.cost == null) continue;
    if (s.class === "scrape") credits += s.cost; // ScrapingBee credits
    else if (s.class === "llm" || s.class === "llm+web") usdMicro += s.cost; // OpenRouter micro-$
  }
  return { credits, usdMicro };
}

/** Cumulative spend across runs. */
export function totalSpend(runs: Check[]): Spend {
  return runs.map((r) => runSpend(r.steps)).reduce(
    (a, b) => ({ credits: a.credits + b.credits, usdMicro: a.usdMicro + b.usdMicro }),
    { credits: 0, usdMicro: 0 },
  );
}
