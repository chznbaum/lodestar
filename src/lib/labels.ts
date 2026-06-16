/** Display-only humanization. Always filter/group/store on the RAW value, never this. */
const OVERRIDES: Record<string, string> = {
  // stages
  pre_seed: "Pre-seed",
  series_c_plus: "Series C+",
  // sizes
  mid_market: "Mid-market",
  // remote
  remote_first: "Remote-first",
  fully_remote: "Fully remote",
  // business models
  b2b: "B2B",
  b2c: "B2C",
  b2b2c: "B2B2C",
  // acronyms / domains
  ai: "AI",
  ml: "ML",
  saas: "SaaS",
  api: "API",
  devops: "DevOps",
};

export function humanize(value: string): string {
  if (!value) return value;
  if (value in OVERRIDES) return OVERRIDES[value];
  return value
    .split("_")
    .map((w) => (w ? w[0].toUpperCase() + w.slice(1) : w))
    .join(" ");
}

/** Humanize a list (e.g. domains, business_model) for display. */
export function humanizeList(values: string[]): string {
  return values.map(humanize).join(", ");
}
