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

/** Human relative date, e.g. relativeDate("2026-06-10","2026-06-15") -> "5d ago". */
export function relativeDate(fromIso: string, todayIso: string): string {
  const from = new Date(fromIso + "T00:00:00");
  const today = new Date(todayIso + "T00:00:00");
  const days = Math.round((today.getTime() - from.getTime()) / 86400000);
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  return `${days}d ago`;
}

/** Humanize a list (e.g. domains, business_model) for display. */
export function humanizeList(values: string[]): string {
  return values.map(humanize).join(", ");
}

/** Two-character monogram for a company logo fallback (e.g. "Stripe" -> "St"). */
export function monogram(name: string): string {
  const n = name.trim();
  return n ? n.slice(0, 2) : "?";
}
