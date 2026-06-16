<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { COMPANY_STATUSES } from "$lib/vault";
  import { humanize, humanizeList } from "$lib/labels";
  import { renderNotes } from "$lib/markdown";

  const slug = $derived(page.params.slug ?? "");
  const company = $derived(cs.bySlug(slug));

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
  });

  let editingNotes = $state(false);
  let notesDraft = $state("");
  $effect(() => { if (!editingNotes) notesDraft = company?.notes ?? ""; });

  async function remove() {
    if (!company) return;
    if (!confirm(`Retire ${company.name}? It stays in the vault as status=removed.`)) return;
    await cs.softRemove(company.slug);
    goto("/");
  }
</script>

<main>
  <p class="crumbs"><a href="/">← Companies</a></p>

  {#if !company}
    <p class="hint">{cs.loading ? "Loading…" : "Company not found."}</p>
  {:else}
    {@const c = company}
    <header>
      <div>
        <h1>{c.name}{#if c.screening}<span class="flag {c.screening}">{c.screening}</span>{/if}</h1>
        <p class="sub">{[humanizeList(c.domain), humanize(c.company_size ?? ""), humanize(c.stage ?? ""), humanize(c.remote_policy ?? ""), c.location].filter(Boolean).join(" · ")}</p>
      </div>
      <span class="spacer"></span>
      {#if c.website}<a class="btn" href={c.website} target="_blank" rel="noreferrer">website ↗</a>{/if}
      {#if c.careers_url}<a class="btn" href={c.careers_url} target="_blank" rel="noreferrer">careers ↗</a>{/if}
      <button class="btn danger" onclick={remove}>Remove…</button>
    </header>

    <div class="statusbar">
      <label>Status
        <select value={c.status ?? ""} onchange={(e) => cs.changeStatus(c.slug, e.currentTarget.value)}>
          {#if !c.status}<option value="" disabled>— unset —</option>{/if}
          {#each COMPANY_STATUSES as s}<option value={s}>{humanize(s)}</option>{/each}
        </select>
      </label>
      <span class="sub">last checked: {c.last_checked ?? "never"}</span>
      <button class="link" onclick={() => cs.markChecked(c.slug)}>mark checked</button>
      <span class="spacer"></span>
      <button class="btn" disabled title="Fetch arrives in a later phase">Fetch jobs (soon)</button>
    </div>

    <section class="panel">
      <div class="ph">Roles found</div>
      <p class="empty">No jobs yet — fetching arrives in a later phase.</p>
    </section>

    <section class="panel">
      <div class="ph">Notes <button class="link" onclick={() => (editingNotes = !editingNotes)}>{editingNotes ? "preview" : "edit"}</button></div>
      {#if editingNotes}
        <textarea class="notes-edit" bind:value={notesDraft}></textarea>
        <button class="btn" disabled={notesDraft === c.notes} onclick={async () => { await cs.saveNotes(c.slug, notesDraft); editingNotes = false; }}>Save notes</button>
      {:else}
        <!-- Trusted (user-authored) content; see markdown.ts security note. -->
        <div class="notes">{@html renderNotes(c.notes)}</div>
      {/if}
    </section>

    <section class="panel">
      <div class="ph">Details</div>
      <dl>
        <dt>Industry</dt><dd>{humanizeList(c.domain) || "—"}{#if c.domain_raw} <span class="sub">({c.domain_raw})</span>{/if}</dd>
        <dt>Model</dt><dd>{humanizeList(c.business_model) || "—"}</dd>
        <dt>Source</dt><dd>{c.source ?? "—"}</dd>
      </dl>
    </section>

    <section class="panel">
      <div class="ph">Activity</div>
      <p class="empty">Check history appears here once the pipeline runs (Phase 3+).</p>
    </section>
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: -apple-system, system-ui, sans-serif; color: #1a1a1a; background: #fafafa; }
  main { max-width: 760px; margin: 0 auto; padding: 1rem 1.5rem 3rem; }
  .crumbs a { color: #2563eb; text-decoration: none; font-size: .85rem; }
  header { display: flex; align-items: flex-start; gap: .5rem; margin: .5rem 0; }
  h1 { font-size: 1.4rem; margin: 0; }
  .sub { color: #666; font-size: .82rem; margin: .15rem 0 0; }
  .spacer { flex: 1; }
  .btn { padding: .3rem .6rem; border: 1px solid #d4d4d4; border-radius: 7px; background: #fff; cursor: pointer; font: inherit; font-size: .82rem; text-decoration: none; color: #1a1a1a; }
  .btn.danger { color: #b91c1c; border-color: #f0c4c4; }
  .btn:disabled { color: #aaa; cursor: default; }
  .statusbar { display: flex; align-items: center; gap: .75rem; margin: .5rem 0 1rem; font-size: .85rem; flex-wrap: wrap; }
  select { padding: .3rem .5rem; border: 1px solid #d4d4d4; border-radius: 7px; }
  .link { border: none; background: none; color: #2563eb; cursor: pointer; font-size: .8rem; padding: 0; }
  .panel { border: 1px solid #e5e5e5; border-radius: 10px; margin-bottom: .75rem; overflow: hidden; }
  .ph { display: flex; justify-content: space-between; padding: .45rem .7rem; background: #f8fafc; border-bottom: 1px solid #eee; font-weight: 600; font-size: .8rem; }
  .panel .empty { color: #999; font-size: .85rem; padding: .6rem .7rem; margin: 0; }
  .notes { padding: .6rem .8rem; font-size: .9rem; }
  .notes-edit { width: 100%; min-height: 8rem; font: inherit; font-size: .85rem; padding: .5rem; border: none; box-sizing: border-box; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: .25rem .9rem; font-size: .85rem; padding: .6rem .8rem; margin: 0; }
  dt { color: #999; } dd { margin: 0; }
  .sub { color: #999; }
  .flag { font-size: .7rem; padding: .1rem .4rem; border-radius: 99px; margin-left: .5rem; vertical-align: middle; }
  .flag.dealbreaker { background: #fee2e2; color: #b91c1c; }
  .flag.caution { background: #fef3c7; color: #b45309; }
  .hint { color: #777; }
</style>
