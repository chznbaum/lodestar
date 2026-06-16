import { describe, it, expect } from "vitest";
import { applyView, type ViewOptions } from "./companyView";
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
