<script lang="ts">
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { getCheck, type Check } from "$lib/check";

  let check = $state<Check | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = page.params.id;
    const vault = cs.vaultPath;
    if (!id || !vault) return;
    getCheck(vault, id)
      .then((c) => {
        check = c;
        error = null;
      })
      .catch((e) => {
        error = String(e);
        check = null;
      });
  });
</script>

{#if error}
  <section class="check-detail"><p class="error">{error}</p></section>
{:else if check}
  <section class="check-detail">
    <a class="check-detail__back" href="/checks">← Checks</a>
    <h1>{check.slug}</h1>
    <p class="check-detail__meta">
      {check.kind} · {check.trigger} · {check.status} ·
      {check.companies.join(", ")} ·
      {check.roles_found} roles · {check.jds_fetched} JDs · {check.errors} errors
    </p>

    <div class="steps__head">
      <span>Stage</span><span>Class</span><span>Target</span>
      <span>Status</span><span>Tries</span><span>Cost</span><span>Error</span>
    </div>
    <ul class="steps__list">
      {#each check.steps as s, i (i)}
        <li class="steps__row steps__row--{s.status}">
          <span>{s.stage}</span>
          <span>{s.class}</span>
          <span>{s.target}</span>
          <span>{s.status}</span>
          <span>{s.attempts}</span>
          <span>{s.cost ?? ""}</span>
          <span>{s.error ?? ""}</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}
