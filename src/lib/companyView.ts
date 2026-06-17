import type { Company } from "./vault";

export type SortKey = "name" | "company_size" | "stage" | "last_checked";

export interface Filters {
  status?: string;
  domain?: string; // matches any of the company's domains
  remote?: string;
  size?: string;
  stage?: string;
  due?: boolean;
}

export interface ViewOptions {
  query: string;
  filters: Filters;
  sort: { key: SortKey; dir: "asc" | "desc" };
  group: boolean;
  resolveDomains?: (slugs: string[]) => DomainInfo[];
}

export interface Group {
  key: string;
  items: Company[];
}

export interface ViewResult {
  flat: Company[];
  groups: Group[] | null;
  ranked: RankedMatch[] | null;
}

export type MatchField = "name" | "domain" | "domain-alias" | "notes";

export interface DomainInfo {
  name: string;
  aliases: string[];
}

export interface RankedMatch {
  company: Company;
  tier: number; // 0 name-prefix · 1 name-substring · 2 domain-name · 3 domain-alias · 4 notes
  field: MatchField;
  domainName?: string; // tier 2 & 3: the owning domain's display name
  alias?: string; // tier 3: the alias that matched
  notesSnippet?: string; // tier 4: windowed snippet around the match
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

function rank(value: string | null, order: string[]): number {
  const i = order.indexOf(value ?? "");
  return i === -1 ? order.length : i;
}

function cmp(a: Company, b: Company, key: SortKey): number {
  switch (key) {
    case "company_size":
      return rank(a.company_size, SIZE_ORDER) - rank(b.company_size, SIZE_ORDER);
    case "stage":
      return rank(a.stage, STAGE_ORDER) - rank(b.stage, STAGE_ORDER);
    case "last_checked":
      return (a.last_checked ?? "").localeCompare(b.last_checked ?? "");
    default:
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  }
}

export function applyView(companies: Company[], opts: ViewOptions): ViewResult {
  const q = opts.query.trim().toLowerCase();
  const f = opts.filters;
  const rows = companies.filter((c) => {
    if (f.status && c.status !== f.status) return false;
    if (f.domain && !c.domain.includes(f.domain)) return false;
    if (f.remote && c.remote_policy !== f.remote) return false;
    if (f.size && c.company_size !== f.size) return false;
    if (f.stage && c.stage !== f.stage) return false;
    if (f.due && !c.due_for_check) return false;
    return true;
  });

  // Results mode: a non-empty query yields a flat, relevance-ranked list;
  // tab grouping and the chosen sort are suspended.
  if (q !== "") {
    const ranked = searchCompanies(rows, opts.query, opts.resolveDomains);
    return { flat: ranked.map((m) => m.company), groups: null, ranked };
  }

  const sorted = [...rows].sort((a, b) => {
    const baseCmp = cmp(a, b, opts.sort.key) || a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    return opts.sort.dir === "desc" ? -baseCmp : baseCmp;
  });

  if (!opts.group) return { flat: sorted, groups: null, ranked: null };

  const byKey = new Map<string, Company[]>();
  for (const c of sorted) {
    const keys = c.domain.length ? c.domain : ["(uncategorized)"];
    for (const k of keys) {
      if (!byKey.has(k)) byKey.set(k, []);
      byKey.get(k)!.push(c);
    }
  }
  const groups = [...byKey.entries()]
    .map(([key, items]) => ({ key, items }))
    .sort((a, b) => a.key.localeCompare(b.key));
  return { flat: sorted, groups, ranked: null };
}

/** Split companies into the action-queue sections (both are subsets of `due_for_check`). */
export function queueSections(companies: Company[]): {
  neverFetched: Company[];
  staleChecked: Company[];
} {
  const due = companies.filter((c) => c.due_for_check);
  return {
    neverFetched: due.filter((c) => !c.last_checked),
    staleChecked: due.filter((c) => !!c.last_checked),
  };
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

/** Default resolver: treat each domain slug as its own name with no aliases.
 *  The app passes a real resolver backed by the domains store. */
const slugAsName = (slugs: string[]): DomainInfo[] => slugs.map((s) => ({ name: s, aliases: [] }));

/** A one-line snippet of `notes` centered on the first match of `q`. */
function notesSnippet(notes: string, q: string, radius = 30): string {
  const idx = notes.toLowerCase().indexOf(q);
  if (idx < 0) return "";
  const start = Math.max(0, idx - radius);
  const end = Math.min(notes.length, idx + q.length + radius);
  const core = notes.slice(start, end).trim();
  return `${start > 0 ? "…" : ""}${core}${end < notes.length ? "…" : ""}`;
}

/** Rank companies by relevance to a free-text query (best/lowest tier first,
 *  name A→Z within a tier). Non-matches are excluded; an empty query → []. */
export function searchCompanies(
  companies: Company[],
  query: string,
  resolveDomains: (slugs: string[]) => DomainInfo[] = slugAsName,
): RankedMatch[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [];

  const matches: RankedMatch[] = [];
  for (const company of companies) {
    const name = company.name.toLowerCase();
    if (name.startsWith(q)) {
      matches.push({ company, tier: 0, field: "name" });
      continue;
    }
    if (name.includes(q)) {
      matches.push({ company, tier: 1, field: "name" });
      continue;
    }
    const domains = resolveDomains(company.domain);
    const nameHit = domains.find((d) => d.name.toLowerCase().includes(q));
    if (nameHit) {
      matches.push({ company, tier: 2, field: "domain", domainName: nameHit.name });
      continue;
    }
    let aliasHit: { domainName: string; alias: string } | null = null;
    for (const d of domains) {
      const alias = d.aliases.find((a) => a.toLowerCase().includes(q));
      if (alias) {
        aliasHit = { domainName: d.name, alias };
        break;
      }
    }
    if (aliasHit) {
      matches.push({ company, tier: 3, field: "domain-alias", domainName: aliasHit.domainName, alias: aliasHit.alias });
      continue;
    }
    if (company.notes.toLowerCase().includes(q)) {
      matches.push({ company, tier: 4, field: "notes", notesSnippet: notesSnippet(company.notes, q) });
    }
  }

  matches.sort(
    (a, b) => a.tier - b.tier || a.company.name.toLowerCase().localeCompare(b.company.name.toLowerCase()),
  );
  return matches;
}
