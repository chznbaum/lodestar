import { listDomains, type Domain } from "$lib/domain";

let domains = $state<Domain[]>([]);
let loadedPath = $state<string | null>(null);
let error = $state<string | null>(null);

export const domainsStore = {
  get domains() {
    return domains;
  },
  get error() {
    return error;
  },
  /** True once domains have been loaded (or attempted) for this vault path. */
  loadedFor(path: string): boolean {
    return loadedPath === path;
  },
  bySlug(slug: string): Domain | null {
    return domains.find((d) => d.slug === slug) ?? null;
  },
  /** Split slugs into the display names of known domains and the slugs that have no note. */
  resolve(slugs: string[]): { names: string[]; unknown: string[] } {
    const names: string[] = [];
    const unknown: string[] = [];
    for (const s of slugs) {
      const d = domains.find((x) => x.slug === s);
      if (d) names.push(d.name);
      else unknown.push(s);
    }
    return { names, unknown };
  },
  async load(vaultPath: string) {
    try {
      domains = await listDomains(vaultPath);
      error = null;
    } catch (e) {
      error = String(e);
      domains = [];
    } finally {
      loadedPath = vaultPath;
    }
  },
};
