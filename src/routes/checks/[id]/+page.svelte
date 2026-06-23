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
      {check.subject} ·
      {check.roles_found} roles · {check.errors} errors
    </p>
    <p class="check-detail__meta">Cost: ScrapingBee <b>{spend.credits}</b> credits · OpenRouter <b>${(spend.usdMicro / 1e6).toFixed(2)}</b></p>

    <table class="steps-table">
      <thead>
        <tr>
          <th>Stage</th>
          <th>Class</th>
          <th>Target</th>
          <th>Status</th>
          <th>Tries</th>
          <th>Cost</th>
          <th>Error</th>
        </tr>
      </thead>
      <tbody>
        {#each check.steps as s, i (i)}
          <tr class={s.status === "failed" ? "steps-row--failed" : ""}>
            <td>{s.stage}</td>
            <td>{s.class}</td>
            <td>{s.target}</td>
            <td>{s.status}</td>
            <td>{s.attempts}</td>
            <td>{fmtCost(s)}</td>
            <td>{s.error ?? ""}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}
