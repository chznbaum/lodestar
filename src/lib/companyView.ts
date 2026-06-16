import type { Company } from "./vault";

export type SortKey = "name" | "company_size" | "stage" | "last_checked";

export interface Filters {
  status?: string;
  industry?: string; // matches any domain
  remote?: string;
  due?: boolean;
}

export interface ViewOptions {
  query: string;
  filters: Filters;
  sort: { key: SortKey; dir: "asc" | "desc" };
  group: boolean;
}

export interface Group {
  key: string;
  items: Company[];
}

export interface ViewResult {
  flat: Company[];
  groups: Group[] | null;
}

const SIZE_ORDER = ["startup", "scaleup", "mid_market", "enterprise"];
const STAGE_ORDER = [
  "pre_seed",
  "seed",
  "series_a",
  "series_b",
  "series_c_plus",
  "public",
  "bootstrapped",
  "unknown",
];

function cmp(a: Company, b: Company, key: SortKey): number {
  switch (key) {
    case "company_size":
      return SIZE_ORDER.indexOf(a.company_size ?? "") - SIZE_ORDER.indexOf(b.company_size ?? "");
    case "stage":
      return STAGE_ORDER.indexOf(a.stage ?? "") - STAGE_ORDER.indexOf(b.stage ?? "");
    case "last_checked":
      return (a.last_checked ?? "").localeCompare(b.last_checked ?? "");
    default:
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  }
}

export function applyView(companies: Company[], opts: ViewOptions): ViewResult {
  const q = opts.query.toLowerCase().trim();
  const f = opts.filters;
  let rows = companies.filter((c) => {
    if (q && !`${c.name} ${c.domain.join(" ")} ${c.notes}`.toLowerCase().includes(q)) return false;
    if (f.status && c.status !== f.status) return false;
    if (f.industry && !c.domain.includes(f.industry)) return false;
    if (f.remote && c.remote_policy !== f.remote) return false;
    if (f.due && !c.due_for_check) return false;
    return true;
  });

  rows = rows.sort((a, b) => {
    const base = cmp(a, b, opts.sort.key) || a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    return opts.sort.dir === "desc" ? -base : base;
  });

  if (!opts.group) return { flat: rows, groups: null };

  const byKey = new Map<string, Company[]>();
  for (const c of rows) {
    const k = c.domain[0] ?? "(uncategorized)";
    if (!byKey.has(k)) byKey.set(k, []);
    byKey.get(k)!.push(c);
  }
  const groups = [...byKey.entries()]
    .map(([key, items]) => ({ key, items }))
    .sort((a, b) => a.key.localeCompare(b.key));
  return { flat: rows, groups };
}

/** Distinct sorted values for building filter dropdowns. */
export function distinct(
  companies: Company[],
  pick: (c: Company) => string[] | string | null,
): string[] {
  const s = new Set<string>();
  for (const c of companies) {
    const v = pick(c);
    if (Array.isArray(v)) v.forEach((x) => x && s.add(x));
    else if (v) s.add(v);
  }
  return [...s].sort();
}
