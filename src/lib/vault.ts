import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface Company {
  slug: string;
  name: string;
  domain: string[];
  business_model: string[];
  status: string | null;
  remote_policy: string | null;
  company_size: string | null;
  stage: string | null;
  location: string | null;
  website: string | null;
  careers_url: string | null;
  last_checked: string | null;
  domain_raw: string | null;
  due_for_check: boolean;
  screening: "dealbreaker" | "caution" | null;
  notes: string;
}

/** Native folder-picker for the vault root. Returns the chosen path, or null if cancelled. */
export async function pickVault(): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title: "Choose your jobsearch-vault folder",
  });
  return typeof result === "string" ? result : null;
}

/** Read + parse every company note under `<vaultPath>/companies`. */
export function listCompanies(vaultPath: string): Promise<Company[]> {
  return invoke<Company[]>("list_companies", { vaultPath });
}

const STATUSES = ["active", "paused", "exhausted", "removed"] as const;
export type CompanyStatus = (typeof STATUSES)[number];
export const COMPANY_STATUSES = STATUSES;

/** Write a single frontmatter field on a company note; returns the re-parsed record. */
export function updateCompanyField(
  vaultPath: string,
  slug: string,
  key: string,
  value: string,
): Promise<Company> {
  return invoke<Company>("update_company_field", { vaultPath, slug, key, value });
}

/** Replace a company note's body; returns the re-parsed record. */
export function setCompanyNotes(vaultPath: string, slug: string, body: string): Promise<Company> {
  return invoke<Company>("set_company_notes", { vaultPath, slug, body });
}

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10); // YYYY-MM-DD
}
