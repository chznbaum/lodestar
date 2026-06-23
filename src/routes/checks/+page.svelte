<script lang="ts">
  import { goto } from "$app/navigation";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { checksStore } from "$lib/checks.svelte";
  import { onRunFinished, onRunStep } from "$lib/pipeline";

  // companiesStore owns the vault path (persisted in localStorage); reuse it here.
  $effect(() => {
    if (cs.vaultPath && !checksStore.loadedFor(cs.vaultPath)) {
      checksStore.load(cs.vaultPath);
    }
  });

  // A run's check-note writes are self-writes (suppressed by the vault watcher to avoid an echo),
  // so the ledger refreshes off the pipeline's own events. Each step reload advances the Steps
  // column live; the terminal event catches final state.

  $effect(() => {
    const subs: (() => void)[] = [];
    let active = true;
    Promise.all([
      onRunStep(() => {
        checksStore.reload();
      }),
      onRunFinished(() => {
        checksStore.reload();
      }),
    ]).then((u) => {
      if (active) subs.push(...u);
      else u.forEach((f) => f());
    });
    return () => {
      active = false;
      subs.forEach((f) => f());
    };
  });

  function open(id: string) {
    goto(`/checks/${id}`);
  }

  // Cumulative spend across loaded runs (per-run totals come from the backend summary).
  const spend = $derived(
    checksStore.checks.reduce(
      (a, c) => ({ credits: a.credits + c.credits, usdMicro: a.usdMicro + c.usd_micro }),
      { credits: 0, usdMicro: 0 },
    ),
  );

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
    <p class="checks__spend">
      ScrapingBee <b>{spend.credits}</b> credits · OpenRouter <b>${(spend.usdMicro / 1e6).toFixed(2)}</b>
      <span class="sub">across {checksStore.checks.length} run{checksStore.checks.length === 1 ? "" : "s"}</span>
    </p>
    <table class="checks-table">
      <thead>
        <tr>
          <th>Started</th>
          <th>Kind</th>
          <th>Trigger</th>
          <th>Subject</th>
          <th>Roles</th>
          <th>Steps</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        {#each checksStore.checks as c (c.slug)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- The anchor in the first cell is the sole keyboard/AT affordance; the row click is mouse-only convenience. -->
          <tr class="checks__row" onclick={() => open(c.slug)}>
            <td><a class="checks__row-link" href="/checks/{c.slug}"><span class="checks__mono">{c.started_at ?? c.slug}</span></a></td>
            <td>{c.kind}</td>
            <td>{c.trigger}</td>
            <td>{c.subject}</td>
            <td>{c.roles_found}</td>
            <td>{c.step_count}{c.failed_count > 0 ? ` (${c.failed_count} failed)` : ""}</td>
            <td><span class="checks__status checks__status--{c.status}">{c.status}</span></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>
