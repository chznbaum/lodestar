import { invoke } from "@tauri-apps/api/core";

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
  source: string | null;
  due_for_check: boolean;
  screening: "dealbreaker" | "caution" | null;
  notes: string;
}

const STATUSES = ["active", "paused", "exhausted", "removed"] as const;
export type CompanyStatus = (typeof STATUSES)[number];
export const COMPANY_STATUSES = STATUSES;

/** Payload for creating a company. Keys are snake_case to match the Rust `NewCompany` serde fields. */
export interface NewCompany {
  name: string;
  website?: string | null;
  careers_url?: string | null;
  domain: string[];
  business_model: string[];
  domain_raw?: string | null;
  company_size?: string | null;
  stage?: string | null;
  remote_policy?: string | null;
  location?: string | null;
  source?: string | null;
  notes: string;
}

/** Read + parse every company note under `<vaultPath>/companies`. */
export function listCompanies(vaultPath: string): Promise<Company[]> {
  return invoke<Company[]>("list_companies", { vaultPath });
}

/** Create a company note; returns the parsed record. Errors if the name yields a taken/empty slug. */
export function createCompany(vaultPath: string, company: NewCompany): Promise<Company> {
  return invoke<Company>("create_company", { vaultPath, company });
}

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

/** Soft-remove / retire: set a validated status (active|paused|exhausted|removed). */
export function setCompanyStatus(vaultPath: string, slug: string, status: string): Promise<Company> {
  return invoke<Company>("set_company_status", { vaultPath, slug, status });
}
