<script lang="ts">
  import { goto } from "$app/navigation";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { domainsStore as ds } from "$lib/domains.svelte";
  import { applyView, distinct, queueSections, type SortKey } from "$lib/companyView";
  import type { Company, Domain } from "$lib/vault";
  import { todayIso } from "$lib/vault";
  import { humanize, monogram, relativeDate } from "$lib/labels";
  import CreateCompanyForm from "$lib/CreateCompanyForm.svelte";
  import Combobox from "$lib/Combobox.svelte";

  type Tab = "queue" | "all" | "domain" | "prospects";
  let tab = $state<Tab>("queue");
  let query = $state("");
  let status = $state("");
  let domainFilter = $state("");
  let remote = $state("");
  let size = $state("");
  let stage = $state("");
  let sortKey = $state<SortKey>("name");
  let sortDir = $state<"asc" | "desc">("asc");
  let showCreate = $state(false);

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
    if (cs.vaultPath && !ds.loadedFor(cs.vaultPath)) ds.load(cs.vaultPath);
  });

  const statuses = $derived(distinct(cs.companies, (c) => c.status));
  const remotes = $derived(distinct(cs.companies, (c) => c.remote_policy));
  const sizes = $derived(distinct(cs.companies, (c) => c.company_size));
  const stages = $derived(distinct(cs.companies, (c) => c.stage));

  const statusOptions = $derived(statuses.map((s) => ({ label: humanize(s), value: s })));
  const domainOptions = $derived(
    distinct(cs.companies, (c) => c.domain)
      .map((slug) => ds.bySlug(slug))
      .filter((d): d is Domain => d !== null)
      .map((d) => ({ label: d.name, value: d.slug, aliases: d.aliases }))
      .sort((a, b) => a.label.localeCompare(b.label)),
  );
  const remoteOptions = $derived(remotes.map((r) => ({ label: humanize(r), value: r })));
  const sizeOptions = $derived(sizes.map((s) => ({ label: humanize(s), value: s })));
  const stageOptions = $derived(stages.map((s) => ({ label: humanize(s), value: s })));

  const view = $derived(
    applyView(cs.companies, {
      query,
      filters: {
        status: status || undefined,
        domain: domainFilter || undefined,
        remote: remote || undefined,
        size: size || undefined,
        stage: stage || undefined,
      },
      sort: { key: sortKey, dir: sortDir },
      group: tab === "domain",
    }),
  );
  const queue = $derived(queueSections(view.flat));

  // E.3 — sortLabel derived
  const sortLabel = $derived.by((): string => {
    if (sortKey === "name") return sortDir === "asc" ? "name A→Z" : "name Z→A";
    if (sortKey === "company_size") return "size";
    if (sortKey === "stage") return "stage";
    if (sortKey === "last_checked") return sortDir === "asc" ? "oldest check" : "newest check";
    return sortKey;
  });

  function open(slug: string) {
    goto(`/companies/${slug}`);
  }
  function setSort(key: SortKey) {
    if (sortKey === key) sortDir = sortDir === "asc" ? "desc" : "asc";
    else { sortKey = key; sortDir = "asc"; }
  }
</script>

<main class="companies">
  <div class="companies__header">
    <h1>Companies <span class="companies__count">{cs.companies.length}</span></h1>
    <input class="companies__search" type="text" placeholder="Search name, domain, notes…" bind:value={query} />
    <button class="btn ghost" onclick={() => (cs.vaultPath ? (showCreate = true) : cs.choose())}>
      {cs.vaultPath ? "+ Add company" : "Choose vault"}
    </button>
  </div>

  <div class="tabs">
    <button
      class="tab"
      class:on={tab === "queue"}
      onclick={() => (tab = "queue")}
    >Queue <span class="tab__count">{queue.neverFetched.length + queue.staleChecked.length} due</span></button>
    <button class="tab" class:on={tab === "all"} onclick={() => (tab = "all")}>All</button>
    <button class="tab" class:on={tab === "domain"} onclick={() => (tab = "domain")}>By domain</button>
    <button class="tab" class:on={tab === "prospects"} onclick={() => (tab = "prospects")}>Best prospects</button>
  </div>

  <div class="companies__filters">
    <Combobox placeholder="Status" bind:value={status} options={statusOptions} />
    <Combobox placeholder="Domain" bind:value={domainFilter} options={domainOptions} />
    <Combobox placeholder="Remote" bind:value={remote} options={remoteOptions} />
    <Combobox placeholder="Size" bind:value={size} options={sizeOptions} />
    <Combobox placeholder="Stage" bind:value={stage} options={stageOptions} />
    <span class="sub">sorted by {sortLabel}</span>
  </div>

  {#if view.flat.length !== cs.companies.length}
    <p class="companies__filter-count hint">Showing {view.flat.length} of {cs.companies.length}</p>
  {/if}

  {#if cs.error}<p class="error">{cs.error}</p>{/if}
  {#if ds.error}<p class="error">Domains: {ds.error}</p>{/if}
  {#if !cs.vaultPath}<p class="hint">Pick your <code>jobsearch-vault</code> folder to begin.</p>
  {:else if cs.loading}<p class="hint">Loading…</p>{/if}

  {#if tab === "queue"}
    {@const q = queue}
    {#if q.neverFetched.length === 0 && q.staleChecked.length === 0}
      <p class="hint">Queue is empty — all companies are up to date.</p>
    {:else}
      <div class="companies__queue-head">
        Never fetched <span class="chip flat chip--count">{q.neverFetched.length}</span>
        <button class="btn primary sm" disabled title="Fetch arrives in a later phase">Fetch all due</button>
      </div>
      {@render qrowsFresh(q.neverFetched)}
      <div class="companies__queue-head">
        Checked &gt; 3 days ago <span class="chip flat chip--count">{q.staleChecked.length}</span>
        <span class="sub">— ages back in here after a fetch</span>
      </div>
      {@render qrowsStale(q.staleChecked)}
    {/if}
  {:else if view.flat.length === 0}
    <p class="hint">No companies match.</p>
  {:else if tab === "domain"}
    {@render thead()}
    {#each view.groups ?? [] as g (g.key)}
      <h2 class="companies__group">{ds.bySlug(g.key)?.name ?? g.key} <span class="companies__count">{g.items.length}</span></h2>
      {@render rows(g.items)}
    {/each}
  {:else if tab === "prospects"}
    {@render thead()}
    <p class="sub companies__prospects-note">Ordering lights up once jobs exist (Phase 1+).</p>
    {@render rows(view.flat)}
  {:else}
    {@render thead()}
    {@render rows(view.flat)}
  {/if}
</main>

{#if showCreate}
  <CreateCompanyForm onclose={() => (showCreate = false)} oncreated={(slug) => { showCreate = false; open(slug); }} />
{/if}

{#snippet thead()}
  <div class="companies__thead">
    <span class="companies__col companies__mono-col"></span>
    <button class="companies__col" onclick={() => setSort("name")}>Name {sortKey === "name" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <span class="companies__col">Domain</span>
    <button class="companies__col" onclick={() => setSort("company_size")}>Size {sortKey === "company_size" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <button class="companies__col" onclick={() => setSort("stage")}>Stage {sortKey === "stage" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <span class="companies__col">Remote</span>
    <span class="companies__col"></span>
  </div>
{/snippet}

{#snippet qrowsFresh(items: Company[])}
  <ul class="companies__list">
    {#each items as c (c.slug)}
      <li class="companies__qrow">
        <span class="monogram">{monogram(c.name)}</span>
        <button class="companies__qrow-body" onclick={() => open(c.slug)}>
          <span class="company-row__name">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}{#each c.business_model as bm}<span class="chip flat">{humanize(bm)}</span>{/each}</span>
          <span class="companies__meta">{ds.resolve(c.domain).names.join(", ")}{#if c.company_size}&nbsp;•&nbsp;{humanize(c.company_size)}{/if}{#if c.remote_policy}&nbsp;•&nbsp;{humanize(c.remote_policy)}{/if}</span>
        </button>
        {#if c.last_checked}
          <span class="sub">checked {relativeDate(c.last_checked, todayIso())}</span>
        {:else}
          <span class="sub">never checked</span>
        {/if}
        <button class="btn primary sm" disabled title="Fetch arrives in a later phase">Fetch jobs</button>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet qrowsStale(items: Company[])}
  <ul class="companies__list">
    {#each items as c (c.slug)}
      <li class="companies__qrow stale">
        <span class="monogram">{monogram(c.name)}</span>
        <button class="companies__qrow-body" onclick={() => open(c.slug)}>
          <span class="company-row__name">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}{#each c.business_model as bm}<span class="chip flat">{humanize(bm)}</span>{/each}</span>
          <span class="companies__meta">{ds.resolve(c.domain).names.join(", ")}{#if c.company_size} · {humanize(c.company_size)}{/if}{#if c.remote_policy} · {humanize(c.remote_policy)}{/if}</span>
        </button>
        {#if c.last_checked}
          <span class="sub">checked {relativeDate(c.last_checked, todayIso())}</span>
        {:else}
          <span class="sub">never checked</span>
        {/if}
        <button class="btn sm" disabled title="Fetch arrives in a later phase">Re-fetch ↻</button>
      </li>
    {/each}
  </ul>
{/snippet}

{#snippet rows(items: Company[])}
  <ul class="companies__list">
    {#each items as c (c.slug)}
      <li>
        <button class="companies__row" onclick={() => open(c.slug)}>
          <span class="monogram">{monogram(c.name)}</span>
          <span class="company-row__name">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}</span>
          <span class="companies__meta">{ds.resolve(c.domain).names.join(", ")}</span>
          <span class="companies__meta">{humanize(c.company_size ?? "")}</span>
          <span class="companies__meta">{humanize(c.stage ?? "")}</span>
          <span class="companies__meta">{humanize(c.remote_policy ?? "")}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}
