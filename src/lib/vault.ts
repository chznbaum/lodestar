import { open } from "@tauri-apps/plugin-dialog";

// Cross-entity vault primitives only. Per-entity types + invoke wrappers live in their
// own modules (company.ts, domain.ts, check.ts, …); collection state lives in the
// per-entity *.svelte.ts stores.

/** Native folder-picker for the vault root. Returns the chosen path, or null if cancelled. */
export async function pickVault(): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title: "Choose your jobsearch-vault folder",
  });
  return typeof result === "string" ? result : null;
}

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10); // YYYY-MM-DD
}
