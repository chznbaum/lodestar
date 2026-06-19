<script lang="ts">
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { getCheck, type Check, type Step } from "$lib/check";
  import { runSpend } from "$lib/spend";

  let check = $state<Check | null>(null);
  let error = $state<string | null>(null);
  const spend = $derived(check ? runSpend(check.steps) : { credits: 0, usdMicro: 0 });

  // Per-step cost, formatted by class: scrape → credits, llm → dollars (sub-cent precision).
  function fmtCost(s: Step): string {
    if (s.cost == null) return "";
    return s.class === "scrape" ? `${s.cost} cr` : `$${(s.cost / 1e6).toFixed(4)}`;
  }

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
      {check.roles_found} roles · {check.errors} errors
    </p>
    <p class="check-detail__meta">Cost: ScrapingBee <b>{spend.credits}</b> credits · OpenRouter <b>${(spend.usdMicro / 1e6).toFixed(2)}</b></p>

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
          <span>{fmtCost(s)}</span>
          <span>{s.error ?? ""}</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}
