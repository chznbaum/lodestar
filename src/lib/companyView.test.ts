import { describe, it, expect } from "vitest";
import { applyView, queueSections, type ViewOptions } from "./companyView";
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

  it("groups by industry (primary domain), sorted group keys", () => {
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
