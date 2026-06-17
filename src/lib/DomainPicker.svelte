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

<div class="domain-picker">
  {#if value.length}
    <ul class="domain-picker__chips">
      {#each value as slug (slug)}
        <li class="domain-picker__chip">
          <span>{ds.bySlug(slug)?.name ?? slug}</span>
          <button type="button" class="domain-picker__chip-remove" aria-label={`Remove ${ds.bySlug(slug)?.name ?? slug}`} onclick={() => remove(slug)}>×</button>
        </li>
      {/each}
    </ul>
  {/if}
  <Combobox placeholder="Add domain" anyLabel="search…" clearable={false} bind:value={pick} options={options} onchange={add} />
</div>
