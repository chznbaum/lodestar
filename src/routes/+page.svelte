<script lang="ts">
  import {
    listCompanies,
    pickVault,
    updateCompanyField,
    setCompanyNotes,
    COMPANY_STATUSES,
    todayIso,
    type Company,
  } from "$lib/vault";
  import { applyView, distinct, type SortKey } from "$lib/companyView";

  let vaultPath = $state<string | null>(
    typeof localStorage !== "undefined" ? localStorage.getItem("vaultPath") : null,
  );
  let companies = $state<Company[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selected = $state<Company | null>(null);

  // view controls
  let query = $state("");
  let status = $state("");
  let industry = $state("");
  let remote = $state("");
  let due = $state(false);
  let sortKey = $state<SortKey>("name");
  let sortDir = $state<"asc" | "desc">("asc");
  let group = $state(false);

  const industries = $derived(distinct(companies, (c) => c.domain));
  const statuses = $derived(distinct(companies, (c) => c.status));
  const remotes = $derived(distinct(companies, (c) => c.remote_policy));

  const view = $derived(
    applyView(companies, {
      query,
      filters: {
        status: status || undefined,
        industry: industry || undefined,
        remote: remote || undefined,
        due: due || undefined,
      },
      sort: { key: sortKey, dir: sortDir },
      group,
    }),
  );

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      companies = await listCompanies(path);
    } catch (e) {
      error = String(e);
      companies = [];
    } finally {
      loading = false;
    }
  }
  async function choose() {
    const path = await pickVault();
    if (!path) return;
    vaultPath = path;
    localStorage.setItem("vaultPath", path);
  }
  $effect(() => {
    if (vaultPath) load(vaultPath);
  });

  // --- editing ---
  function applyUpdate(updated: Company) {
    companies = companies.map((c) => (c.slug === updated.slug ? updated : c));
    if (selected?.slug === updated.slug) selected = updated;
  }
  async function changeStatus(c: Company, value: string) {
    applyUpdate(await updateCompanyField(vaultPath!, c.slug, "status", value));
  }
  async function markChecked(c: Company) {
    applyUpdate(await updateCompanyField(vaultPath!, c.slug, "last_checked", todayIso()));
  }
  let notesDraft = $state("");
  $effect(() => {
    notesDraft = selected?.notes ?? "";
  });
  async function saveNotes(c: Company) {
    applyUpdate(await setCompanyNotes(vaultPath!, c.slug, notesDraft));
  }
</script>

<div class="app">
  <main>
    <header>
      <h1>Companies <span class="count">{view.flat.length}/{companies.length}</span></h1>
      <input class="search" placeholder="Search name, industry, notes…" bind:value={query} />
      <button onclick={choose}>{vaultPath ? "Change vault" : "Choose vault"}</button>
    </header>

    <div class="controls">
      <select bind:value={status}><option value="">status: any</option>{#each statuses as s}<option>{s}</option>{/each}</select>
      <select bind:value={industry}><option value="">industry: any</option>{#each industries as i}<option>{i}</option>{/each}</select>
      <select bind:value={remote}><option value="">remote: any</option>{#each remotes as r}<option>{r}</option>{/each}</select>
      <label><input type="checkbox" bind:checked={due} /> due for check</label>
      <span class="spacer"></span>
      <select bind:value={sortKey}>
        <option value="name">sort: name</option>
        <option value="company_size">sort: size</option>
        <option value="stage">sort: stage</option>
        <option value="last_checked">sort: last checked</option>
      </select>
      <button onclick={() => (sortDir = sortDir === "asc" ? "desc" : "asc")}>{sortDir === "asc" ? "↑" : "↓"}</button>
      <label><input type="checkbox" bind:checked={group} /> group by industry</label>
    </div>

    {#if error}<p class="error">{error}</p>{/if}
    {#if !vaultPath}<p class="hint">Pick your <code>jobsearch-vault</code> folder to begin.</p>{:else if loading}<p class="hint">Loading…</p>{/if}

    {#if view.groups}
      {#each view.groups as g (g.key)}
        <h2 class="group">{g.key} <span class="count">{g.items.length}</span></h2>
        {@render rows(g.items)}
      {/each}
    {:else}
      {@render rows(view.flat)}
    {/if}
  </main>

  {#if selected}
    {@const c = selected}
    <aside class="detail">
      <button class="close" onclick={() => (selected = null)}>✕</button>
      <h2>{c.name}{#if c.screening}<span class="flag {c.screening}">{c.screening}</span>{/if}</h2>
      <dl>
        <dt>Industry</dt><dd>{c.domain.join(", ") || "—"}</dd>
        <dt>Size · Stage</dt><dd>{[c.company_size, c.stage].filter(Boolean).join(" · ") || "—"}</dd>
        <dt>Remote</dt><dd>{c.remote_policy ?? "—"}</dd>
        <dt>Location</dt><dd>{c.location ?? "—"}</dd>
        <dt>Status</dt>
        <dd>
          <select value={c.status ?? ""} onchange={(e) => changeStatus(c, e.currentTarget.value)}>
            {#each COMPANY_STATUSES as s}<option value={s}>{s}</option>{/each}
          </select>
          {#if c.due_for_check} · due{/if}
        </dd>
        <dt>Last checked</dt>
        <dd>{c.last_checked ?? "never"} <button class="link" onclick={() => markChecked(c)}>mark checked</button></dd>
        <dt>Links</dt>
        <dd>
          {#if c.careers_url}<a href={c.careers_url} target="_blank" rel="noreferrer">careers</a>{/if}
          {#if c.website}<a href={c.website} target="_blank" rel="noreferrer">site</a>{/if}
        </dd>
      </dl>
      <h3>Notes</h3>
      <textarea class="notes-edit" bind:value={notesDraft}></textarea>
      <button onclick={() => saveNotes(c)} disabled={notesDraft === c.notes}>Save notes</button>
    </aside>
  {/if}
</div>

{#snippet rows(items: Company[])}
  <ul class="list">
    {#each items as c (c.slug)}
      <li>
        <button class="row" class:active={selected?.slug === c.slug} onclick={() => (selected = c)}>
          <span class="name">{c.name}{#if c.screening === "dealbreaker"}<span class="dot" title="dealbreaker">⛔</span>{/if}</span>
          <span class="meta">{c.domain.join(", ")}</span>
          <span class="meta">{[c.company_size, c.stage].filter(Boolean).join(" · ")}</span>
          <span class="meta">{c.remote_policy ?? ""}</span>
          {#if c.due_for_check}<span class="due">due</span>{/if}
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  :global(body) { margin: 0; font-family: -apple-system, system-ui, sans-serif; color: #1a1a1a; background: #fafafa; }
  .app { display: flex; }
  main { flex: 1; max-width: 1000px; margin: 0 auto; padding: 1rem 1.5rem 3rem; }
  header { display: flex; gap: .75rem; align-items: center; }
  h1 { font-size: 1.25rem; margin: 0; white-space: nowrap; }
  .count { color: #999; font-weight: 400; font-size: .85rem; }
  .search { flex: 1; padding: .45rem .7rem; border: 1px solid #d4d4d4; border-radius: 7px; }
  .controls { display: flex; gap: .5rem; align-items: center; flex-wrap: wrap; margin: .75rem 0; font-size: .85rem; }
  .controls .spacer { flex: 1; }
  select, button { padding: .35rem .6rem; border: 1px solid #d4d4d4; border-radius: 7px; background: #fff; cursor: pointer; font-size: .85rem; }
  .group { font-size: .95rem; margin: 1rem 0 .25rem; color: #444; }
  .list { list-style: none; margin: 0; padding: 0; }
  .list li { border-bottom: 1px solid #eee; }
  .row { display: grid; grid-template-columns: 16rem 1fr 11rem 7rem auto; gap: 1rem; align-items: baseline; width: 100%; padding: .5rem .25rem; border: none; border-radius: 0; background: none; cursor: pointer; text-align: left; font: inherit; }
  .row:hover { background: #f0f0f0; }
  .row.active { background: #e8eefc; }
  .name { font-weight: 600; }
  .meta { color: #666; font-size: .88rem; }
  .due { color: #b45309; font-size: .78rem; background: #fef3c7; padding: .1rem .45rem; border-radius: 99px; justify-self: start; }
  .dot { margin-left: .35rem; font-size: .8rem; }
  .error { color: #b91c1c; }
  .hint { color: #777; }
  .detail { width: 22rem; border-left: 1px solid #e5e5e5; padding: 1rem 1.25rem; height: 100vh; position: sticky; top: 0; overflow-y: auto; background: #fff; }
  .detail .close { float: right; border: none; background: none; font-size: 1rem; }
  .detail dl { display: grid; grid-template-columns: auto 1fr; gap: .25rem .75rem; font-size: .85rem; }
  .detail dt { color: #999; }
  .detail dd { margin: 0; }
  .notes-edit { width: 100%; min-height: 8rem; font: inherit; font-size: .85rem; padding: .5rem; border: 1px solid #d4d4d4; border-radius: 7px; resize: vertical; box-sizing: border-box; }
  .link { border: none; background: none; color: #2563eb; cursor: pointer; font-size: .8rem; padding: 0; }
  .flag { font-size: .7rem; padding: .1rem .4rem; border-radius: 99px; margin-left: .5rem; vertical-align: middle; }
  .flag.dealbreaker { background: #fee2e2; color: #b91c1c; }
  .flag.caution { background: #fef3c7; color: #b45309; }
</style>
