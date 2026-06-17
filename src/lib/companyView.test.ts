import { describe, it, expect } from "vitest";
import { applyView, queueSections, searchCompanies, type ViewOptions, type DomainInfo } from "./companyView";
import type { Company } from "./vault";

const mk = (over: Partial<Company>): Company => ({
  slug: "x",
  name: "X",
  domain: [],
  business_model: [],
  status: "active",
  remote_policy: null,
  company_size: null,
  stage: null,
  location: null,
  website: null,
  careers_url: null,
  last_checked: null,
  domain_raw: null,
  source: null,
  due_for_check: false,
  screening: null,
  notes: "",
  ...over,
});

const data: Company[] = [
  mk({ slug: "b", name: "Beta", domain: ["healthcare"], company_size: "startup" }),
  mk({ slug: "a", name: "Alpha", domain: ["fintech"], company_size: "enterprise", status: "paused" }),
  mk({ slug: "c", name: "Gamma", domain: ["healthcare"], due_for_check: true }),
];

const base: ViewOptions = {
  query: "",
  filters: {},
  sort: { key: "name", dir: "asc" },
  group: false,
};

describe("applyView", () => {
  it("searches across name + domain + notes", () => {
    const r = applyView(data, { ...base, query: "fin" });
    expect(r.flat.map((c) => c.slug)).toEqual(["a"]);
  });

  it("filters by status and due", () => {
    const r = applyView(data, { ...base, filters: { status: "active", due: true } });
    expect(r.flat.map((c) => c.slug)).toEqual(["c"]);
  });

  it("sorts by name desc", () => {
    const r = applyView(data, { ...base, sort: { key: "name", dir: "desc" } });
    expect(r.flat.map((c) => c.name)).toEqual(["Gamma", "Beta", "Alpha"]);
  });

  it("groups by domain, sorted group keys", () => {
    const r = applyView(data, { ...base, group: true });
    expect(r.groups!.map((g) => g.key)).toEqual(["fintech", "healthcare"]);
    expect(r.groups!.find((g) => g.key === "healthcare")!.items.map((c) => c.slug)).toEqual(["b", "c"]);
  });
});

it("groups a multi-domain company under each of its domains", () => {
  const cs = [
    mk({ slug: "a", name: "A", domain: ["ai", "healthcare"] }),
    mk({ slug: "b", name: "B", domain: ["ai"] }),
    mk({ slug: "c", name: "C", domain: [] }),
  ];
  const { groups } = applyView(cs, {
    query: "", filters: {}, sort: { key: "name", dir: "asc" }, group: true,
  });
  const byKey = Object.fromEntries(groups!.map((g) => [g.key, g.items.map((i) => i.slug)]));
  expect(byKey["ai"].sort()).toEqual(["a", "b"]);
  expect(byKey["healthcare"]).toEqual(["a"]);
  expect(byKey["(uncategorized)"]).toEqual(["c"]);
});

it("filters by size and stage", () => {
  const cs = [
    mk({ slug: "a", name: "A", company_size: "startup", stage: "seed" }),
    mk({ slug: "b", name: "B", company_size: "enterprise", stage: "public" }),
  ];
  const out = applyView(cs, {
    query: "", filters: { size: "startup" }, sort: { key: "name", dir: "asc" }, group: false,
  });
  expect(out.flat.map((c) => c.slug)).toEqual(["a"]);
  const out2 = applyView(cs, {
    query: "", filters: { stage: "public" }, sort: { key: "name", dir: "asc" }, group: false,
  });
  expect(out2.flat.map((c) => c.slug)).toEqual(["b"]);
});

it("sorts companies with unknown size after known sizes (ascending)", () => {
  const cs = [
    mk({ slug: "unknown", name: "A", company_size: null }),
    mk({ slug: "known", name: "B", company_size: "startup" }),
  ];
  const out = applyView(cs, {
    query: "", filters: {}, sort: { key: "company_size", dir: "asc" }, group: false,
  });
  expect(out.flat.map((c) => c.slug)).toEqual(["known", "unknown"]);
});

it("queueSections splits due companies into never-fetched vs stale-checked", () => {
  const cs = [
    mk({ slug: "never", name: "N", due_for_check: true, last_checked: null }),
    mk({ slug: "stale", name: "S", due_for_check: true, last_checked: "2026-01-01" }),
    mk({ slug: "fresh", name: "F", due_for_check: false, last_checked: "2026-06-15" }),
  ];
  const { neverFetched, staleChecked } = queueSections(cs);
  expect(neverFetched.map((c) => c.slug)).toEqual(["never"]);
  expect(staleChecked.map((c) => c.slug)).toEqual(["stale"]);
});

describe("searchCompanies", () => {
  const resolve = (slugs: string[]): DomainInfo[] =>
    slugs.map((s) => {
      if (s === "financial_services") return { name: "Financial Services", aliases: ["fintech"] };
      if (s === "security") return { name: "Security", aliases: ["infosec"] };
      return { name: s, aliases: [] };
    });

  const cos = [
    mk({ slug: "numeric", name: "Numeric", domain: ["financial_services"], notes: "AI accounting platform" }),
    mk({ slug: "vega", name: "Vega Security", domain: ["security"] }),
    mk({ slug: "cogent", name: "Cogent Security", domain: ["security"] }),
    mk({ slug: "sardine", name: "Sardine", domain: ["financial_services", "security"] }),
  ];

  it("returns [] for an empty or whitespace query", () => {
    expect(searchCompanies(cos, "", resolve)).toEqual([]);
    expect(searchCompanies(cos, "   ", resolve)).toEqual([]);
  });

  it("ranks a name prefix as tier 0", () => {
    const r = searchCompanies(cos, "vega", resolve);
    expect(r.map((m) => m.company.slug)).toEqual(["vega"]);
    expect(r[0]).toMatchObject({ tier: 0, field: "name" });
  });

  it("ranks a name substring (non-prefix) as tier 1", () => {
    const r = searchCompanies(cos, "ardin", resolve); // "Sardine"
    expect(r.map((m) => m.company.slug)).toEqual(["sardine"]);
    expect(r[0]).toMatchObject({ tier: 1, field: "name" });
  });

  it("matches on a resolved domain name (tier 2) and records which name", () => {
    const r = searchCompanies(cos, "financial", resolve); // domain name, no name hit
    expect(r.map((m) => m.company.slug)).toEqual(["numeric", "sardine"]); // name A→Z
    expect(r[0]).toMatchObject({ tier: 2, field: "domain", domainName: "Financial Services" });
  });

  it("matches on a domain alias (tier 3) and records the alias + owning domain", () => {
    const r = searchCompanies(cos, "fintech", resolve);
    expect(r.map((m) => m.company.slug)).toEqual(["numeric", "sardine"]);
    expect(r[0]).toMatchObject({ tier: 3, field: "domain-alias", domainName: "Financial Services", alias: "fintech" });
  });

  it("matches on notes (tier 4) with a snippet around the hit", () => {
    const r = searchCompanies(cos, "accounting", resolve);
    expect(r.map((m) => m.company.slug)).toEqual(["numeric"]);
    expect(r[0]).toMatchObject({ tier: 4, field: "notes" });
    expect(r[0].notesSnippet).toContain("accounting");
  });

  it("orders by tier, then name A→Z within a tier", () => {
    const r = searchCompanies(cos, "security", resolve);
    // Vega/Cogent match on name (tier 1, A→Z), Sardine on domain name (tier 2)
    expect(r.map((m) => m.company.slug)).toEqual(["cogent", "vega", "sardine"]);
    expect(r.map((m) => m.tier)).toEqual([1, 1, 2]);
  });

  it("excludes companies that match nothing", () => {
    expect(searchCompanies(cos, "zzz", resolve)).toEqual([]);
  });
});

describe("applyView results mode", () => {
  it("enters results mode for a non-empty query (ranked set, groups null, sort/group suspended)", () => {
    const cs = [
      mk({ slug: "vega", name: "Vega Security", domain: ["security"] }),
      mk({ slug: "cogent", name: "Cogent Security", domain: ["security"] }),
    ];
    const r = applyView(cs, { ...base, query: "security", group: true, sort: { key: "name", dir: "desc" } });
    expect(r.groups).toBeNull();
    expect(r.ranked).not.toBeNull();
    // ranked (tier1, name A→Z) — NOT the requested name-desc sort
    expect(r.ranked!.map((m) => m.company.slug)).toEqual(["cogent", "vega"]);
    expect(r.flat.map((c) => c.slug)).toEqual(["cogent", "vega"]);
  });

  it("ranks domain matches below name matches, and honors a custom resolver", () => {
    const cs = [
      mk({ slug: "sardine", name: "Sardine", domain: ["security"] }), // domain → tier 2
      mk({ slug: "vega", name: "Vega Security", domain: ["other"] }),  // name → tier 1
    ];
    const resolveDomains = (slugs: string[]) =>
      slugs.map((s) => ({ name: s === "security" ? "Security" : s, aliases: [] }));
    const r = applyView(cs, { ...base, query: "security", resolveDomains });
    expect(r.flat.map((c) => c.slug)).toEqual(["vega", "sardine"]);
  });

  it("leaves ranked null and applies normal sort/group when the query is empty", () => {
    const r = applyView(data, { ...base, group: true });
    expect(r.ranked).toBeNull();
    expect(r.groups).not.toBeNull();
  });
});
