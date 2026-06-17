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

<main>
  <div class="mhead">
    <h1>Companies <span class="count">{cs.companies.length}</span></h1>
    <input class="search" placeholder="Search name, domain, notes…" bind:value={query} />
    <button class="btn ghost" onclick={() => (cs.vaultPath ? (showCreate = true) : cs.choose())}>
      {cs.vaultPath ? "+ Add company" : "Choose vault"}
    </button>
  </div>

  <div class="tabs">
    <button
      class="tab"
      class:on={tab === "queue"}
      onclick={() => (tab = "queue")}
    >Queue <span class="tab-count">{queue.neverFetched.length + queue.staleChecked.length} due</span></button>
    <button class="tab" class:on={tab === "all"} onclick={() => (tab = "all")}>All</button>
    <button class="tab" class:on={tab === "domain"} onclick={() => (tab = "domain")}>By domain</button>
    <button class="tab" class:on={tab === "prospects"} onclick={() => (tab = "prospects")}>Best prospects</button>
  </div>

  <div class="filters">
    <Combobox placeholder="Status" bind:value={status} options={statusOptions} />
    <Combobox placeholder="Domain" bind:value={domainFilter} options={domainOptions} />
    <Combobox placeholder="Remote" bind:value={remote} options={remoteOptions} />
    <Combobox placeholder="Size" bind:value={size} options={sizeOptions} />
    <Combobox placeholder="Stage" bind:value={stage} options={stageOptions} />
    <span class="grow"></span>
    <span class="sub">sorted by {sortLabel}</span>
  </div>

  {#if view.flat.length !== cs.companies.length}
    <p class="filter-count hint">Showing {view.flat.length} of {cs.companies.length}</p>
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
      <div class="secthdr" style="border-top: none;">
        Never fetched <span class="badge">{q.neverFetched.length}</span>
        <span class="grow"></span>
        <button class="btn primary sm" disabled title="Fetch arrives in a later phase">Fetch all due</button>
      </div>
      {@render qrowsFresh(q.neverFetched)}
      <div class="secthdr">
        Checked &gt; 3 days ago <span class="badge">{q.staleChecked.length}</span>
        <span class="sub">— ages back in here after a fetch</span>
      </div>
      {@render qrowsStale(q.staleChecked)}
    {/if}
  {:else if view.flat.length === 0}
    <p class="hint">No companies match.</p>
  {:else if tab === "domain"}
    {@render thead()}
    {#each view.groups ?? [] as g (g.key)}
      <h2 class="group">{ds.bySlug(g.key)?.name ?? g.key} <span class="count">{g.items.length}</span></h2>
      {@render rows(g.items)}
    {/each}
  {:else if tab === "prospects"}
    {@render thead()}
    <p class="sub prospects-note">Ordering lights up once jobs exist (Phase 1+).</p>
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
  <div class="thead">
    <span class="col mono-col"></span>
    <button class="col" onclick={() => setSort("name")}>Name {sortKey === "name" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <span class="col">Domain</span>
    <button class="col" onclick={() => setSort("company_size")}>Size {sortKey === "company_size" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <button class="col" onclick={() => setSort("stage")}>Stage {sortKey === "stage" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
    <span class="col">Remote</span>
    <span class="col"></span>
  </div>
{/snippet}

{#snippet qrowsFresh(items: Company[])}
  <ul class="list">
    {#each items as c (c.slug)}
      <li class="qrow">
        <span class="monogram">{monogram(c.name)}</span>
        <button class="qrow-body" onclick={() => open(c.slug)}>
          <span class="nm">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}{#each c.business_model as bm}<span class="chip flat">{humanize(bm)}</span>{/each}</span>
          <span class="meta">{ds.resolve(c.domain).names.join(", ")}{#if c.company_size}&nbsp;•&nbsp;{humanize(c.company_size)}{/if}{#if c.remote_policy}&nbsp;•&nbsp;{humanize(c.remote_policy)}{/if}</span>
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
  <ul class="list">
    {#each items as c (c.slug)}
      <li class="qrow stale">
        <span class="monogram">{monogram(c.name)}</span>
        <button class="qrow-body" onclick={() => open(c.slug)}>
          <span class="nm">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}{#each c.business_model as bm}<span class="chip flat">{humanize(bm)}</span>{/each}</span>
          <span class="meta">{ds.resolve(c.domain).names.join(", ")}{#if c.company_size} · {humanize(c.company_size)}{/if}{#if c.remote_policy} · {humanize(c.remote_policy)}{/if}</span>
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
  <ul class="list">
    {#each items as c (c.slug)}
      <li>
        <button class="row" onclick={() => open(c.slug)}>
          <span class="monogram">{monogram(c.name)}</span>
          <span class="name">{c.name}{#if c.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if c.screening === "caution"}<span class="chip warn">caution</span>{/if}</span>
          <span class="meta">{ds.resolve(c.domain).names.join(", ")}</span>
          <span class="meta">{humanize(c.company_size ?? "")}</span>
          <span class="meta">{humanize(c.stage ?? "")}</span>
          <span class="meta">{humanize(c.remote_policy ?? "")}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  main {
    padding: 1rem var(--sp-content) 1.2rem;
    min-width: 0;
  }

  /* ── Header ───────────────────────────────────────────── */
  .mhead {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-bottom: var(--sp-3);
  }
  h1 {
    font-family: var(--font-display);
    font-size: var(--fs-mhead);
    font-variation-settings: "opsz" 60, "wght" 600;
    margin: 0;
    white-space: nowrap;
  }
  .count {
    background: var(--accent-ink);
    color: var(--accent-soft);
    border-radius: var(--r-pill);
    padding: 0.12rem 0.5rem;
    font-weight: 700;
    font-size: var(--fs-xs);
  }
  .search {
    flex: 1;
    border: 1px solid var(--wire);
    border-radius: var(--r-md);
    padding: 0.32rem 0.6rem;
    color: var(--muted);
    font: inherit;
    font-size: var(--fs-sm);
    background: var(--card);
    min-width: 6rem;
  }
  .search:focus {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  /* ── Tabs ─────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: var(--sp-1);
    border-bottom: 1px solid var(--line);
    margin-bottom: 0.6rem;
  }
  .tab {
    font: inherit;
    font-size: var(--fs-sm);
    padding: 0.4rem 0.6rem;
    color: var(--muted);
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
    margin-bottom: -1px;
  }
  .tab.on {
    color: var(--primary);
    border-bottom-color: var(--primary);
    font-weight: 700;
  }
  .tab-count {
    font-size: var(--fs-xs);
    color: var(--accent-ink);
    font-weight: 700;
  }

  /* ── Filters ──────────────────────────────────────────── */
  .filters {
    display: flex;
    gap: var(--sp-1);
    flex-wrap: wrap;
    align-items: center;
    margin-bottom: 0.6rem;
    font-size: var(--fs-xs);
  }
  .filter-count {
    font-size: var(--fs-xs);
    margin: 0 0 var(--sp-2);
  }

  /* ── Section headers (queue) ──────────────────────────── */
  .secthdr {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 0.55rem 0.15rem 0.45rem;
    color: var(--ink-soft);
    font-weight: 700;
    font-size: var(--fs-sm);
    border-top: 1px solid var(--line);
  }
  .badge {
    background: var(--flat-soft);
    border-radius: var(--r-pill);
    padding: 0.1rem 0.45rem;
    font-size: 0.7rem;
    color: var(--muted);
    font-weight: 600;
  }
  .sub {
    color: var(--faint);
    font-size: var(--fs-xs);
    font-weight: 400;
  }
  .grow {
    flex: 1;
  }

  /* ── Queue rows ───────────────────────────────────────── */
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .qrow {
    display: grid;
    grid-template-columns: var(--monogram-size) 1fr auto auto;
    gap: 0.65rem;
    align-items: center;
    padding: var(--sp-row-y) 0.2rem;
    border-bottom: 1px solid var(--line);
  }
  .qrow.stale {
    opacity: 0.5;
  }
  .qrow-body {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.1rem;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .qrow-body:hover .nm {
    color: var(--primary);
  }
  .nm {
    font-family: var(--font-display);
    font-size: var(--fs-co);
    font-weight: 600;
    font-variation-settings: "opsz" 11, "wght" 600;
    color: var(--ink);
    line-height: 1.1;
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex-wrap: wrap;
  }
  .qrow .meta {
    color: var(--muted);
    font-size: var(--fs-xs);
  }

  /* ── All / Domain / Prospects rows ───────────────────── */
  .group {
    font-size: var(--fs-co);
    margin: var(--sp-4) 0 var(--sp-1);
    color: var(--ink-soft);
    font-weight: 600;
  }
  .prospects-note {
    margin: 0.2rem 0 var(--sp-2);
    color: var(--primary);
  }
  .thead {
    display: grid;
    grid-template-columns: var(--monogram-size) 16rem 1fr 9rem 9rem 7rem auto;
    gap: var(--sp-4);
    padding: var(--sp-row-y) 0.2rem;
    border-bottom: 2px solid var(--line);
    font-size: var(--fs-xs);
    color: var(--faint);
  }
  .thead .col {
    background: none;
    border: none;
    text-align: left;
    padding: 0;
    color: var(--faint);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .thead .mono-col {
    cursor: default;
  }
  .list li {
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: grid;
    grid-template-columns: var(--monogram-size) 16rem 1fr 9rem 9rem 7rem auto;
    gap: var(--sp-4);
    align-items: center;
    width: 100%;
    padding: var(--sp-row-y) 0.2rem;
    border: none;
    border-radius: 0;
    background: none;
    font: inherit;
    text-align: left;
    cursor: pointer;
    color: var(--ink);
  }
  .row:hover {
    background: var(--card-hover);
  }
  .name {
    font-family: var(--font-display);
    font-size: var(--fs-co);
    font-weight: 600;
    font-variation-settings: "opsz" 11, "wght" 600;
    line-height: 1.1;
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex-wrap: wrap;
  }
  .meta {
    color: var(--muted);
    font-size: var(--fs-sm);
  }
</style>
