<script lang="ts">
  import { goto } from "$app/navigation";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { applyView, distinct, queueSections, type SortKey } from "$lib/companyView";
  import type { Company } from "$lib/vault";
  import { humanize, humanizeList } from "$lib/labels";
  import CreateCompanyForm from "$lib/CreateCompanyForm.svelte";

  type Tab = "queue" | "all" | "domain" | "prospects";
  let tab = $state<Tab>("queue");
  let query = $state("");
  let status = $state("");
  let industry = $state("");
  let remote = $state("");
  let size = $state("");
  let stage = $state("");
  let sortKey = $state<SortKey>("name");
  let sortDir = $state<"asc" | "desc">("asc");
  let showCreate = $state(false);

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
  });

  const industries = $derived(distinct(cs.companies, (c) => c.domain));
  const statuses = $derived(distinct(cs.companies, (c) => c.status));
  const remotes = $derived(distinct(cs.companies, (c) => c.remote_policy));
  const sizes = $derived(distinct(cs.companies, (c) => c.company_size));
  const stages = $derived(distinct(cs.companies, (c) => c.stage));

  const view = $derived(
    applyView(cs.companies, {
      query,
      filters: {
        status: status || undefined,
        industry: industry || undefined,
        remote: remote || undefined,
        size: size || undefined,
        stage: stage || undefined,
      },
      sort: { key: sortKey, dir: sortDir },
      group: tab === "domain",
    }),
  );
  const queue = $derived(queueSections(view.flat));

  function open(slug: string) {
    goto(`/companies/${slug}`);
  }
  function setSort(key: SortKey) {
    if (sortKey === key) sortDir = sortDir === "asc" ? "desc" : "asc";
    else { sortKey = key; sortDir = "asc"; }
  }
</script>

<main>
  <header>
    <h1>Companies <span class="count">{view.flat.length}/{cs.companies.length}</span></h1>
    <input class="search" placeholder="Search name, industry, notes…" bind:value={query} />
    <button onclick={() => (cs.vaultPath ? (showCreate = true) : cs.choose())}>
      {cs.vaultPath ? "+ Add company" : "Choose vault"}
    </button>
  </header>

  <nav class="tabs">
    <button class:on={tab === "queue"} onclick={() => (tab = "queue")}>Queue</button>
    <button class:on={tab === "all"} onclick={() => (tab = "all")}>All</button>
    <button class:on={tab === "domain"} onclick={() => (tab = "domain")}>By domain</button>
    <button class:on={tab === "prospects"} onclick={() => (tab = "prospects")}>Best prospects</button>
  </nav>

  <div class="controls">
    <select bind:value={status}><option value="">status: any</option>{#each statuses as s}<option value={s}>{humanize(s)}</option>{/each}</select>
    <select bind:value={industry}><option value="">industry: any</option>{#each industries as i}<option value={i}>{humanize(i)}</option>{/each}</select>
    <select bind:value={remote}><option value="">remote: any</option>{#each remotes as r}<option value={r}>{humanize(r)}</option>{/each}</select>
    <select bind:value={size}><option value="">size: any</option>{#each sizes as s}<option value={s}>{humanize(s)}</option>{/each}</select>
    <select bind:value={stage}><option value="">stage: any</option>{#each stages as s}<option value={s}>{humanize(s)}</option>{/each}</select>
  </div>

  {#if cs.error}<p class="error">{cs.error}</p>{/if}
  {#if !cs.vaultPath}<p class="hint">Pick your <code>jobsearch-vault</code> folder to begin.</p>
  {:else if cs.loading}<p class="hint">Loading…</p>{/if}

  {#if tab === "queue"}
    {@const q = queue}
    {#if q.neverFetched.length === 0 && q.staleChecked.length === 0}
      <p class="hint">Queue is empty — all companies are up to date.</p>
    {:else}
      <h2 class="group">Never fetched <span class="count">{q.neverFetched.length}</span></h2>
      {@render rows(q.neverFetched)}
      <h2 class="group">Checked &gt; 3 days ago <span class="count">{q.staleChecked.length}</span></h2>
      {@render rows(q.staleChecked)}
    {/if}
  {:else if view.flat.length === 0}
    <p class="hint">No companies match.</p>
  {:else if tab === "domain"}
    {#each view.groups ?? [] as g (g.key)}
      <h2 class="group">{humanize(g.key)} <span class="count">{g.items.length}</span></h2>
      {@render rows(g.items)}
    {/each}
  {:else if tab === "prospects"}
    <p class="hint">Best-prospects ordering lights up once jobs exist (Phase 1+). For now, showing all by name.</p>
    {@render rows(view.flat)}
  {:else}
    <div class="thead">
      <button class="col" onclick={() => setSort("name")}>Name {sortKey === "name" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
      <span class="col">Industry</span>
      <button class="col" onclick={() => setSort("company_size")}>Size {sortKey === "company_size" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
      <button class="col" onclick={() => setSort("stage")}>Stage {sortKey === "stage" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
      <span class="col">Remote</span>
      <span class="col"></span>
    </div>
    {@render rows(view.flat)}
  {/if}
</main>

{#if showCreate}
  <CreateCompanyForm onclose={() => (showCreate = false)} oncreated={(slug) => { showCreate = false; open(slug); }} />
{/if}

{#snippet rows(items: Company[])}
  <ul class="list">
    {#each items as c (c.slug)}
      <li>
        <button class="row" onclick={() => open(c.slug)}>
          <span class="name">{c.name}{#if c.screening === "dealbreaker"}<span class="dot" title="dealbreaker">⛔</span>{:else if c.screening === "caution"}<span class="dot" title="caution">⚠️</span>{/if}</span>
          <span class="meta">{humanizeList(c.domain)}</span>
          <span class="meta">{humanize(c.company_size ?? "")}</span>
          <span class="meta">{humanize(c.stage ?? "")}</span>
          <span class="meta">{humanize(c.remote_policy ?? "")}</span>
          {#if c.due_for_check}<span class="due">due</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  :global(body) { margin: 0; font-family: -apple-system, system-ui, sans-serif; color: #1a1a1a; background: #fafafa; }
  main { max-width: 1040px; margin: 0 auto; padding: 1rem 1.5rem 3rem; }
  header { display: flex; gap: .75rem; align-items: center; }
  h1 { font-size: 1.25rem; margin: 0; white-space: nowrap; }
  .count { color: #999; font-weight: 400; font-size: .85rem; }
  .search { flex: 1; padding: .45rem .7rem; border: 1px solid #d4d4d4; border-radius: 7px; }
  button { padding: .35rem .6rem; border: 1px solid #d4d4d4; border-radius: 7px; background: #fff; cursor: pointer; font: inherit; font-size: .85rem; }
  .tabs { display: flex; gap: .25rem; margin: .75rem 0 .5rem; }
  .tabs button { border-radius: 7px 7px 0 0; color: #555; }
  .tabs button.on { color: #2563eb; border-color: #c7d7fb; background: #e8eefc; font-weight: 600; }
  .controls { display: flex; gap: .5rem; flex-wrap: wrap; margin: .25rem 0 .75rem; font-size: .85rem; }
  select { padding: .35rem .6rem; border: 1px solid #d4d4d4; border-radius: 7px; background: #fff; }
  .group { font-size: .95rem; margin: 1rem 0 .25rem; color: #444; }
  .thead { display: grid; grid-template-columns: 16rem 1fr 9rem 9rem 7rem auto; gap: 1rem; padding: .35rem .25rem; border-bottom: 2px solid #e5e5e5; font-size: .78rem; color: #888; }
  .thead .col { background: none; border: none; text-align: left; padding: 0; color: #888; cursor: pointer; }
  .list { list-style: none; margin: 0; padding: 0; }
  .list li { border-bottom: 1px solid #eee; }
  .row { display: grid; grid-template-columns: 16rem 1fr 9rem 9rem 7rem auto; gap: 1rem; align-items: baseline; width: 100%; padding: .5rem .25rem; border: none; border-radius: 0; background: none; text-align: left; }
  .row:hover { background: #f0f0f0; }
  .name { font-weight: 600; }
  .meta { color: #666; font-size: .88rem; }
  .due { color: #b45309; font-size: .78rem; background: #fef3c7; padding: .1rem .45rem; border-radius: 99px; justify-self: start; }
  .dot { margin-left: .35rem; font-size: .8rem; }
  .error { color: #b91c1c; }
  .hint { color: #777; }
</style>
