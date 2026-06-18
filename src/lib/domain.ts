import { invoke } from "@tauri-apps/api/core";

export interface Domain {
  slug: string;
  name: string;
  aliases: string[];
  screening: "dealbreaker" | "caution" | null;
}

/** Read + parse every domain note under `<vaultPath>/domains`. */
export function listDomains(vaultPath: string): Promise<Domain[]> {
  return invoke<Domain[]>("list_domains", { vaultPath });
}
