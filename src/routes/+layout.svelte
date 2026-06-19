<script lang="ts">
  import "$lib/styles/index.css";
  import "@fontsource-variable/bodoni-moda";
  import "@fontsource/lora/400.css";
  import "@fontsource/lora/500.css";
  import "@fontsource/lora/600.css";
  import "@fontsource/spline-sans/400.css";
  import "@fontsource/spline-sans/500.css";
  import "@fontsource/spline-sans/600.css";
  import "@fontsource/spline-sans/700.css";
  import { page } from "$app/state";
  import { companiesStore } from "$lib/companies.svelte";
  import { startVaultSync } from "$lib/vaultSync";

  let { children } = $props();

  // One app-lifetime subscription that live-reloads stores when vault notes change on disk
  // outside the app (e.g. edited in Obsidian). Re-runs if the chosen vault path changes; the
  // backend watcher and the frontend listener are both torn down/restarted on change.
  $effect(() => {
    const path = companiesStore.vaultPath;
    if (!path) return;
    let unlisten: (() => void) | undefined;
    let active = true;
    startVaultSync(path).then((fn) => (active ? (unlisten = fn) : fn()));
    return () => {
      active = false;
      unlisten?.();
    };
  });

  // Companies is the only built surface; it owns "/" and "/companies/*".
  const onCompanies = $derived(
    page.url.pathname === "/" || page.url.pathname.startsWith("/companies"),
  );
  const onChecks = $derived(page.url.pathname.startsWith("/checks"));
  const onSettings = $derived(page.url.pathname.startsWith("/settings"));
</script>

<div class="app">
  <nav class="rail">
    <div class="rail__brand">Lodestar</div>
    <span class="navlink future">Today</span>
    <span class="navlink future">Triage</span>
    <span class="navlink future">Pipeline</span>
    <a class="navlink" class:on={onCompanies} href="/">Companies</a>
    <span class="navlink future">Network</span>
    <span class="navlink future">Patterns</span>
    <div class="rail__sep"></div>
    <div class="rail__util">Diagnostics</div>
    <a class="navlink" class:on={onChecks} href="/checks">Checks</a>
    <a class="navlink" class:on={onSettings} href="/settings">Settings</a>
  </nav>

  <div class="app__content">
    {@render children()}
  </div>
</div>
