<script lang="ts">
  import { tick } from "svelte";
  import { goto } from "$app/navigation";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { domainsStore as ds } from "$lib/domains.svelte";
  import { applyView, distinct, queueSections, type SortKey, type RankedMatch } from "$lib/companyView";
  import { segments } from "$lib/highlight";
  import type { Company } from "$lib/company";
  import type { Domain } from "$lib/domain";
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
  let activeIndex = $state(-1);

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
      resolveDomains: (slugs: string[]) =>
        slugs.map((s) => {
          const d = ds.bySlug(s);
          return d ? { name: d.name, aliases: d.aliases ?? [] } : { name: s, aliases: [] };
        }),
    }),
  );
  const queue = $derived(queueSections(view.flat));
  $effect(() => {
    query;
    view.ranked;
    activeIndex = -1;
  });

  const searching = $derived(query.trim() !== "");
  const tabLabel = $derived(
    tab === "queue" ? "Queue" : tab === "all" ? "All" : tab === "domain" ? "By domain" : "Best prospects",
  );

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

  async function scrollActiveIntoView() {
    await tick();
    if (activeIndex < 0) return;
    const slug = (view.ranked ?? [])[activeIndex]?.company.slug;
    if (slug) document.getElementById(`result-${slug}`)?.scrollIntoView({ block: "nearest" });
  }

  function onSearchKey(e: KeyboardEvent) {
    const matches = view.ranked ?? [];
    if (e.key === "Escape") {
      e.preventDefault();
      query = "";
      activeIndex = -1;
      return;
    }
    if (!searching || matches.length === 0) return;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        activeIndex = activeIndex >= matches.length - 1 ? 0 : activeIndex + 1;
        scrollActiveIntoView();
        break;
      case "ArrowUp":
        e.preventDefault();
        activeIndex = activeIndex <= 0 ? matches.length - 1 : activeIndex - 1;
        scrollActiveIntoView();
        break;
      case "Home":
        e.preventDefault();
        activeIndex = 0;
        scrollActiveIntoView();
        break;
      case "End":
        e.preventDefault();
        activeIndex = matches.length - 1;
        scrollActiveIntoView();
        break;
      case "Enter":
        if (activeIndex >= 0) {
          e.preventDefault();
          open(matches[activeIndex].company.slug);
        }
        break;
    }
  }

  function setSort(key: SortKey) {
    if (sortKey === key) sortDir = sortDir === "asc" ? "desc" : "asc";
    else { sortKey = key; sortDir = "asc"; }
  }
</script>

<main class="companies">
  <div class="companies__header">
    <h1>Companies <span class="companies__count">{cs.companies.length}</span></h1>
    <div class="companies__search-wrap">
      <input
        class="companies__search"
        type="text"
        placeholder="Search name, domain, notes…"
        bind:value={query}
        onkeydown={onSearchKey}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={searching}
        aria-controls="company-search-results"
        aria-activedescendant={activeIndex >= 0 && view.ranked
          ? `result-${view.ranked[activeIndex].company.slug}`
          : undefined}
      />
      {#if searching}<span class="companies__search-hint">↑↓ move · ↵ open · esc clear</span>{/if}
    </div>
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

  {#if searching}
    <p class="companies__filter-count hint">
      <b>{(view.ranked ?? []).length}</b> companies match "{query}" — ranked ·
      <button class="linkbtn" onclick={() => (query = "")}>clear</button> to return to {tabLabel}
    </p>
  {:else if view.flat.length !== cs.companies.length}
    <p class="companies__filter-count hint">Showing {view.flat.length} of {cs.companies.length}</p>
  {/if}

  {#if cs.error}<p class="error">{cs.error}</p>{/if}
  {#if ds.error}<p class="error">Domains: {ds.error}</p>{/if}
  {#if !cs.vaultPath}<p class="hint">Pick your <code>jobsearch-vault</code> folder to begin.</p>
  {:else if cs.loading}<p class="hint">Loading…</p>{/if}

  {#if searching}
    {#if (view.ranked ?? []).length === 0}
      <div class="companies__empty">
        <div class="companies__empty-ico">∅</div>
        <p class="companies__empty-title">No companies match "{query}"</p>
        <p class="companies__empty-hint">Searched name, domain, and notes.</p>
        <button class="btn sm" onclick={() => (query = "")}>↺ Clear search</button>
      </div>
    {:else}
      {@render resultsList(view.ranked ?? [])}
    {/if}
  {:else if tab === "queue"}
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

{#snippet hl(text: string)}{#each segments(text, query) as s}{#if s.mark}<mark>{s.text}</mark>{:else}{s.text}{/if}{/each}{/snippet}

{#snippet resultsList(matches: RankedMatch[])}
  <div class="companies__rthead">
    <span></span><span>Name</span><span>Domain</span><span>Size</span><span>Remote</span>
  </div>
  <ul class="companies__list" role="listbox" aria-label="Company search results" id="company-search-results">
    {#each matches as m, i (m.company.slug)}
      <!-- svelte-ignore a11y_click_events_have_key_events -- keyboard nav is centralized on the search input via aria-activedescendant (W3C combobox/listbox pattern) -->
      <li
        id="result-{m.company.slug}"
        class="companies__result"
        class:is-active={i === activeIndex}
        role="option"
        aria-selected={i === activeIndex}
        onclick={() => open(m.company.slug)}
        onmouseenter={() => (activeIndex = i)}
      >
        <span class="monogram">{monogram(m.company.name)}</span>
        <span class="company-row__name">
          {#if m.field === "name"}{@render hl(m.company.name)}{:else}{m.company.name}{/if}
          {#if m.company.screening === "dealbreaker"}<span class="chip danger">dealbreaker</span>{:else if m.company.screening === "caution"}<span class="chip warn">caution</span>{/if}
        </span>
        <span class="companies__meta">
          {#if m.field === "domain"}
            {#each ds.resolve(m.company.domain).names as dn, di}{#if di > 0}, {/if}{#if dn === m.domainName}{@render hl(dn)}{:else}{dn}{/if}{/each}
          {:else}
            {ds.resolve(m.company.domain).names.join(", ")}
          {/if}
        </span>
        <span class="companies__meta">{humanize(m.company.company_size ?? "")}</span>
        <span class="companies__meta">{humanize(m.company.remote_policy ?? "")}</span>
        {#if m.field === "domain-alias" && m.domainName && m.alias}
          <span class="companies__reason">matches domain {m.domainName} (alias "{@render hl(m.alias)}")</span>
        {:else if m.field === "notes" && m.notesSnippet}
          <span class="companies__reason">{@render hl(m.notesSnippet)}</span>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}
