import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { companiesStore } from "$lib/companies.svelte";
import { checksStore } from "$lib/checks.svelte";
import { domainsStore } from "$lib/domains.svelte";

/**
 * A vault note created/modified on disk from OUTSIDE the app (e.g. edited in Obsidian). The
 * backend file-watcher suppresses the app's own writes, so this only fires for genuine external
 * edits — never as an echo of our own saves or pipeline writes. `kind` is the note's entity type;
 * `slug` its filename stem.
 */
export interface RecordChanged {
  kind: "company" | "job" | "domain" | "check" | (string & {});
  slug: string;
}

/** Subscribe to external vault changes. Returns an unlisten fn (call it on teardown). */
export function onRecordChanged(cb: (e: RecordChanged) => void): Promise<UnlistenFn> {
  return listen<RecordChanged>("record:changed", (ev) => cb(ev.payload));
}

/**
 * Start backend file-watching for `vaultPath` and route external changes to the matching global
 * store. Call once from the root layout; calling again with a new path re-points the backend
 * watcher. The returned fn tears down the frontend listener.
 *
 * Jobs are intentionally NOT routed here: they have no global store and surface only on the
 * company workspace, which subscribes via `onRecordChanged` to refresh its own list. Pipeline-
 * driven updates (a run writing checks/jobs) flow through the `run:*` events instead — see
 * pipeline.ts — which is why the watcher suppresses those self-writes.
 */
export async function startVaultSync(vaultPath: string): Promise<UnlistenFn> {
  await invoke("start_vault_watcher", { vaultPath });
  return onRecordChanged((e) => {
    switch (e.kind) {
      case "company":
        companiesStore.load();
        break;
      case "domain":
        domainsStore.reload();
        break;
      case "check":
        checksStore.reload();
        break;
    }
  });
}
