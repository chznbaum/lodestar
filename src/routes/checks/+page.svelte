<script lang="ts">
  import { goto } from "$app/navigation";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { checksStore } from "$lib/checks.svelte";

  // companiesStore owns the vault path (persisted in localStorage); reuse it here.
  $effect(() => {
    if (cs.vaultPath && !checksStore.loadedFor(cs.vaultPath)) {
      checksStore.load(cs.vaultPath);
    }
  });

  function open(id: string) {
    goto(`/checks/${id}`);
  }
</script>

<section class="checks">
  <h1>Checks</h1>

  {#if !cs.vaultPath}
    <p class="hint">Choose your <code>jobsearch-vault</code> folder on the Companies page first.</p>
  {:else if checksStore.error}
    <p class="error">{checksStore.error}</p>
  {:else if checksStore.checks.length === 0}
    <p class="hint">No runs yet. Fetching jobs will record runs here.</p>
  {:else}
    <div class="checks__head">
      <span>Started</span><span>Kind</span><span>Trigger</span>
      <span>Companies</span><span>Roles</span><span>Steps</span><span>Status</span>
    </div>
    <ul class="checks__list">
      {#each checksStore.checks as c (c.slug)}
        <li>
          <button class="checks__row" onclick={() => open(c.slug)}>
            <span class="checks__mono">{c.started_at ?? c.slug}</span>
            <span>{c.kind}</span>
            <span>{c.trigger}</span>
            <span>{c.company_count}</span>
            <span>{c.roles_found}</span>
            <span>{c.step_count}{c.failed_count > 0 ? ` (${c.failed_count} failed)` : ""}</span>
            <span class="checks__status checks__status--{c.status}">{c.status}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>
