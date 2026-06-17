<script lang="ts">
  import Combobox from "$lib/Combobox.svelte";
  import { domainsStore as ds } from "$lib/domains.svelte";
  import type { ComboOption } from "$lib/combobox";

  let { value = $bindable() }: { value: string[] } = $props();

  const options = $derived<ComboOption[]>(
    ds.domains
      .filter((d) => !value.includes(d.slug))
      .map((d) => ({ label: d.name, value: d.slug, aliases: d.aliases }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );

  let pick = $state("");

  function add(slug: string) {
    if (slug && !value.includes(slug)) value = [...value, slug];
    pick = ""; // reset the add-control back to its prompt
  }
  function remove(slug: string) {
    value = value.filter((s) => s !== slug);
  }
</script>

<div class="picker">
  {#if value.length}
    <ul class="chips">
      {#each value as slug (slug)}
        <li class="dchip">
          <span>{ds.bySlug(slug)?.name ?? slug}</span>
          <button type="button" class="x" aria-label={`Remove ${ds.bySlug(slug)?.name ?? slug}`} onclick={() => remove(slug)}>×</button>
        </li>
      {/each}
    </ul>
  {/if}
  <Combobox placeholder="Add domain" anyLabel="search…" clearable={false} bind:value={pick} options={options} onchange={add} />
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    margin-top: 0.15rem;
  }
  .chips {
    list-style: none;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
    margin: 0;
    padding: 0;
  }
  .dchip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: var(--flat-soft);
    color: var(--ink-soft);
    border-radius: var(--r-pill);
    padding: 0.12rem 0.2rem 0.12rem 0.55rem;
    font-size: var(--fs-xs);
  }
  .dchip .x {
    border: none;
    background: none;
    cursor: pointer;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1;
    padding: 0 0.2rem;
  }
  .dchip .x:hover {
    color: var(--primary);
  }
</style>
