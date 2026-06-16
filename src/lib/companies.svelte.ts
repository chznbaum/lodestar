import {
  listCompanies,
  pickVault,
  updateCompanyField,
  setCompanyNotes,
  setCompanyStatus,
  createCompany,
  todayIso,
  type Company,
  type NewCompany,
} from "$lib/vault";

let vaultPath = $state<string | null>(
  typeof localStorage !== "undefined" ? localStorage.getItem("vaultPath") : null,
);
let companies = $state<Company[]>([]);
let loading = $state(false);
let loaded = $state(false);
let error = $state<string | null>(null);

function byName(a: Company, b: Company) {
  return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
}
function apply(updated: Company) {
  companies = companies.map((c) => (c.slug === updated.slug ? updated : c));
}

export const companiesStore = {
  get vaultPath() { return vaultPath; },
  get companies() { return companies; },
  get loading() { return loading; },
  get loaded() { return loaded; },
  get error() { return error; },

  bySlug(slug: string): Company | null {
    return companies.find((c) => c.slug === slug) ?? null;
  },

  async load() {
    if (!vaultPath) return;
    loading = true;
    error = null;
    try {
      companies = await listCompanies(vaultPath);
    } catch (e) {
      error = String(e);
      companies = [];
    } finally {
      loading = false;
      loaded = true;
    }
  },

  async choose() {
    const path = await pickVault();
    if (!path) return;
    vaultPath = path;
    loaded = false;
    localStorage.setItem("vaultPath", path);
    await this.load();
  },

  async changeStatus(slug: string, status: string) {
    apply(await setCompanyStatus(vaultPath!, slug, status));
  },
  async markChecked(slug: string) {
    apply(await updateCompanyField(vaultPath!, slug, "last_checked", todayIso()));
  },
  async saveNotes(slug: string, body: string) {
    apply(await setCompanyNotes(vaultPath!, slug, body));
  },
  /** Soft-remove: retire by status (note is kept). */
  async softRemove(slug: string, status: "removed" | "exhausted" = "removed") {
    apply(await setCompanyStatus(vaultPath!, slug, status));
  },
  async create(nc: NewCompany): Promise<Company> {
    const c = await createCompany(vaultPath!, nc);
    companies = [...companies, c].sort(byName);
    return c;
  },
};
