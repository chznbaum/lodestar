import { listChecks, type CheckSummary } from "$lib/check";

let checks = $state<CheckSummary[]>([]);
let loadedPath = $state<string | null>(null);
let error = $state<string | null>(null);

export const checksStore = {
  get checks() {
    return checks;
  },
  get error() {
    return error;
  },
  /** True once checks have been loaded (or attempted) for this vault path. */
  loadedFor(path: string): boolean {
    return loadedPath === path;
  },
  byId(id: string): CheckSummary | null {
    return checks.find((c) => c.slug === id) ?? null;
  },
  async load(vaultPath: string) {
    try {
      checks = await listChecks(vaultPath);
      error = null;
    } catch (e) {
      error = String(e);
      checks = [];
    } finally {
      loadedPath = vaultPath;
    }
  },
};
