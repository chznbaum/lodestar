<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { COMPANY_STATUSES } from "$lib/vault";
  import { humanize, humanizeList } from "$lib/labels";
  import { renderNotes } from "$lib/markdown";
  import Combobox from "$lib/Combobox.svelte";

  const slug = $derived(page.params.slug ?? "");
  const company = $derived(cs.bySlug(slug));

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
  });

  let editingNotes = $state(false);
  let notesDraft = $state("");
  $effect(() => { if (!editingNotes) notesDraft = company?.notes ?? ""; });

  let editingDetails = $state(false);
  let detailDraft = $state<Record<string, string>>({});

  function openDetailEdit() {
    const c = company;
    if (!c) return;
    detailDraft = {
      domain: c.domain.join(", "),
      business_model: c.business_model.join(", "),
      company_size: c.company_size ?? "",
      stage: c.stage ?? "",
      remote_policy: c.remote_policy ?? "",
      location: c.location ?? "",
      website: c.website ?? "",
      careers_url: c.careers_url ?? "",
      domain_raw: c.domain_raw ?? "",
      source: c.source ?? "",
    };
    editingDetails = true;
  }

  function cancelDetailEdit() {
    editingDetails = false;
  }

  function fmtList(raw: string): string {
    const arr = raw.split(",").map((s) => s.trim()).filter(Boolean);
    return "[" + arr.join(", ") + "]";
  }

  function currentFmtList(arr: string[]): string {
    return "[" + arr.join(", ") + "]";
  }

  async function saveDetails() {
    const c = company;
    if (!c) return;
    const writes: Array<[string, string]> = [];

    // List fields
    for (const key of ["domain", "business_model"] as const) {
      const formatted = fmtList(detailDraft[key] ?? "");
      const current = currentFmtList(c[key]);
      if (formatted !== current) writes.push([key, formatted]);
    }

    // Scalar fields
    const scalarKeys = ["company_size", "stage", "remote_policy", "location", "website", "careers_url", "domain_raw", "source"] as const;
    for (const key of scalarKeys) {
      const draftVal = detailDraft[key] ?? "";
      const currentVal = (c[key] ?? "") as string;
      if (draftVal !== currentVal) writes.push([key, draftVal]);
    }

    for (const [key, value] of writes) {
      await cs.updateField(c.slug, key, value);
    }
    editingDetails = false;
  }

  const statusOptions = COMPANY_STATUSES.map((s) => ({ label: humanize(s), value: s }));
  const sizeOptions = ["startup", "scaleup", "mid_market", "enterprise"].map((s) => ({ label: humanize(s), value: s }));
  const stageOptions = ["pre_seed", "seed", "series_a", "series_b", "series_c_plus", "public", "bootstrapped", "unknown"].map((s) => ({ label: humanize(s), value: s }));
  const remoteOptions = ["fully_remote", "remote_first", "hybrid", "onsite", "unknown"].map((r) => ({ label: humanize(r), value: r }));

  let wtab = $state<"overview" | "roles" | "activity">("overview");

  async function remove() {
    if (!company) return;
    if (!confirm(`Retire ${company.name}? It stays in the vault as status=removed.`)) return;
    await cs.softRemove(company.slug);
    goto("/");
  }
</script>

<div class="workspace">
  {#if !company}
    <p class="crumbs"><a href="/">← Companies</a></p>
    <p class="hint">{cs.loading ? "Loading…" : "Company not found."}</p>
  {:else}
    {@const c = company}

    <p class="crumbs"><a href="/">← Companies</a> / {c.name}</p>

    <header class="whead">
      <div class="wname-block">
        <h1 class="wname">{c.name}</h1>
        <p class="wsub">{[humanizeList(c.domain), humanize(c.company_size ?? ""), humanize(c.stage ?? ""), humanize(c.remote_policy ?? ""), c.location].filter(Boolean).join(" · ")}</p>
      </div>
      <span class="spacer"></span>
      {#if c.screening}
        {#if c.screening === "dealbreaker"}
          <span class="chip danger">{c.screening}</span>
        {:else}
          <span class="chip warn">{c.screening}</span>
        {/if}
      {:else}
        <span class="chip good">no flags</span>
      {/if}
      <div class="wactions">
        {#if c.website}<a class="btn sm" href={c.website} target="_blank" rel="noreferrer">website ↗</a>{/if}
        {#if c.careers_url}<a class="btn sm" href={c.careers_url} target="_blank" rel="noreferrer">careers ↗</a>{/if}
        <button class="btn sm danger" onclick={remove}>Remove…</button>
      </div>
    </header>

    <div class="statusbar">
      <Combobox
        placeholder="Status"
        value={c.status ?? ""}
        clearable={false}
        options={statusOptions}
        onchange={(v) => cs.changeStatus(c.slug, v)}
      />
      <span class="sub">last checked: {c.last_checked ?? "never checked"}</span>
      <button class="linkbtn" onclick={() => cs.markChecked(c.slug)}>mark checked</button>
      <span class="spacer"></span>
      <button class="btn sm" disabled title="Fetch arrives in a later phase">Fetch jobs</button>
    </div>

    <div class="tabs">
      <button class="t" class:on={wtab === "overview"} onclick={() => (wtab = "overview")}>Overview</button>
      <button class="t" class:on={wtab === "roles"} onclick={() => (wtab = "roles")}>Roles</button>
      <button class="t" class:on={wtab === "activity"} onclick={() => (wtab = "activity")}>Activity</button>
    </div>

    {#if wtab === "overview"}
      <section class="panel">
        <div class="ph">Roles found</div>
        {#if !c.last_checked}
          <p class="empty">Not fetched yet — "Fetch jobs" will list matching roles here.</p>
        {:else}
          <p class="empty">No active roles found as of {c.last_checked}.</p>
        {/if}
      </section>

      <section class="panel">
        <div class="ph">
          <span>Notes <span class="sub">rendered markdown</span></span>
          <button class="linkbtn" onclick={() => (editingNotes = !editingNotes)}>{editingNotes ? "reject" : "edit"}</button>
        </div>
        {#if editingNotes}
          <textarea class="notes-edit" bind:value={notesDraft}></textarea>
          <div class="notes-save-row">
            <button class="btn sm" disabled={notesDraft === c.notes} onclick={async () => { await cs.saveNotes(c.slug, notesDraft); editingNotes = false; }}>Save notes</button>
          </div>
        {:else}
          <!-- Trusted (user-authored) content; see markdown.ts security note. -->
          <div class="notes">{@html renderNotes(c.notes)}</div>
        {/if}
      </section>

      <section class="panel">
        <div class="ph">
          Details
          {#if editingDetails}
            <span>
              <button class="linkbtn" onclick={cancelDetailEdit}>cancel</button>
              <button class="btn sm" onclick={saveDetails}>Save</button>
            </span>
          {:else}
            <button class="linkbtn" onclick={openDetailEdit}>edit ✎</button>
          {/if}
        </div>
        {#if editingDetails}
          <div class="pb">
            <div class="detail-edit">
              <label class="de-label" for="de-domain">Industry</label>
              <input id="de-domain" class="de-input" type="text" bind:value={detailDraft.domain} placeholder="e.g. ai, fintech" />

              <label class="de-label" for="de-business_model">Model</label>
              <input id="de-business_model" class="de-input" type="text" bind:value={detailDraft.business_model} placeholder="e.g. saas, marketplace" />

              <label class="de-label" for="de-company_size">Size</label>
              <Combobox id="de-company_size" placeholder="Size" bind:value={detailDraft.company_size} options={sizeOptions} />
              <label class="de-label" for="de-stage">Stage</label>
              <Combobox id="de-stage" placeholder="Stage" bind:value={detailDraft.stage} options={stageOptions} />
              <label class="de-label" for="de-remote_policy">Remote</label>
              <Combobox id="de-remote_policy" placeholder="Remote" bind:value={detailDraft.remote_policy} options={remoteOptions} />

              <label class="de-label" for="de-location">Location</label>
              <input id="de-location" class="de-input" type="text" bind:value={detailDraft.location} />

              <label class="de-label" for="de-website">Website</label>
              <input id="de-website" class="de-input" type="text" bind:value={detailDraft.website} />

              <label class="de-label" for="de-careers_url">Careers</label>
              <input id="de-careers_url" class="de-input" type="text" bind:value={detailDraft.careers_url} />

              <label class="de-label" for="de-domain_raw">Industry (raw)</label>
              <input id="de-domain_raw" class="de-input" type="text" bind:value={detailDraft.domain_raw} />

              <label class="de-label" for="de-source">Source</label>
              <input id="de-source" class="de-input" type="text" bind:value={detailDraft.source} />
            </div>
          </div>
        {:else}
          <div class="pb">
            <dl class="meta-grid">
              <dt>Industry</dt><dd>{humanizeList(c.domain) || "—"}{#if c.domain_raw}&nbsp;<span class="sub-sm">({c.domain_raw})</span>{/if}</dd>
              <dt>Model</dt><dd>{humanizeList(c.business_model) || "—"}</dd>
              <dt>Size</dt><dd>{humanize(c.company_size ?? "") || "—"}</dd>
              <dt>Stage</dt><dd>{humanize(c.stage ?? "") || "—"}</dd>
              <dt>Remote</dt><dd>{humanize(c.remote_policy ?? "") || "—"}</dd>
              <dt>Location</dt><dd>{c.location ?? "—"}</dd>
              <dt>Website</dt><dd style="font-family: monospace;">{c.website ?? "—"}</dd>
              <dt>Careers</dt><dd style="font-family: monospace;">{c.careers_url ?? "—"}</dd>
              <dt>Source</dt><dd style="font-family: monospace;">{c.source ?? "—"}</dd>
            </dl>
          </div>
        {/if}
      </section>
    {/if}

    {#if wtab === "roles"}
      <section class="panel">
        <div class="ph">Roles found</div>
        {#if !c.last_checked}
          <p class="empty">Not fetched yet — "Fetch jobs" will list matching roles here.</p>
        {:else}
          <p class="empty">No active roles found as of {c.last_checked}.</p>
        {/if}
      </section>
    {/if}

    {#if wtab === "activity"}
      <section class="panel">
        <div class="ph">Activity</div>
        <p class="empty">Check history appears here once the pipeline runs (Phase 3+).</p>
      </section>
    {/if}
  {/if}
</div>

<style>
  /* ── layout wrapper ── */
  .workspace {
    padding: 1rem var(--sp-content) 1.2rem;
  }

  /* ── breadcrumbs ── */
  .crumbs {
    font-size: var(--fs-xs);
    color: var(--muted);
    margin: 0 0 var(--sp-2);
  }
  .crumbs a {
    color: var(--primary);
    text-decoration: none;
  }

  /* ── header ── */
  .whead {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    flex-wrap: wrap;
  }
  .wname-block {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--sp-2);
  }
  .wname {
    font-family: var(--font-display);
    font-size: var(--fs-wname);
    font-weight: 700;
    font-variation-settings: "opsz" 72, "wght" 700;
    margin: 0;
    line-height: 1.2;
  }
  .wsub {
    width: 100%;
    color: var(--muted);
    font-size: var(--fs-sm);
    margin: 0.1rem 0 0;
  }
  .spacer { flex: 1; }
  .wactions {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  /* ── status bar ── */
  .statusbar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin: 0.7rem 0 0.2rem;
    flex-wrap: wrap;
    font-size: var(--fs-sm);
  }
  .sub {
    color: var(--faint);
    font-size: var(--fs-xs);
  }

  span .sub {
    margin-left: 0.5rem;
  }

  .sub-sm {
    font-size: var(--fs-xs);
  }

  /* ── tabs ── */
  .tabs {
    display: flex;
    gap: var(--sp-1);
    border-bottom: 1px solid var(--line);
    margin-bottom: 0.6rem;
  }
  .t {
    font-size: var(--fs-sm);
    padding: 0.32rem 0.6rem;
    color: var(--muted);
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
    line-height: 1.5;
    margin-bottom: -1px;
  }
  .t.on {
    color: var(--primary);
    border-bottom-color: var(--primary);
    font-weight: 700;
  }

  /* ── card panels ── */
  .panel {
    border: 1px solid var(--line);
    border-radius: var(--r-panel);
    margin-top: var(--sp-3);
    overflow: hidden;
  }
  .ph {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--panel-head);
    border-bottom: 1px solid var(--line);
    font-weight: 600;
    font-size: var(--fs-panel-head);
    color: var(--ink);
  }
  .pb {
    padding: 0.7rem 0.8rem;
  }
  .panel .empty {
    color: var(--faint);
    font-size: var(--fs-sm);
    padding: var(--sp-2) 0.7rem;
    margin: 0;
  }

  /* ── notes ── */
  .notes {
    padding: var(--sp-2) 0.8rem;
    font-family: var(--font-body);
    font-size: var(--fs-panel-body);
    color: var(--ink-soft);
  }
  .notes :global(p) { margin: 0.4rem 0; }
  .notes :global(p:first-child) { margin-top: 0; }
  .notes :global(ul) { margin: 0.4rem 0; padding-left: 1.1rem; }
  .notes-edit {
    display: block;
    width: 100%;
    min-height: 8rem;
    font: inherit;
    font-size: var(--fs-sm);
    padding: var(--sp-2);
    border: none;
    border-bottom: 1px solid var(--line);
    box-sizing: border-box;
    resize: vertical;
  }
  .notes-save-row {
    padding: var(--sp-2) 0.7rem;
  }

  /* ── details dl ── */
  .meta-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.7rem 1.1rem;
    font-size: var(--fs-md);
    margin: 0;
    align-items: baseline;
  }
  .meta-grid dt {
    color: var(--muted);
    font-size: var(--fs-sm);
  }
  .meta-grid dd {
    margin: 0;
    font-family: var(--font-body);
    color: var(--ink);
  }

  /* ── details edit form ── */
  .detail-edit {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.35rem 0.75rem;
    align-items: center;
  }
  .de-label {
    font-size: var(--fs-sm);
    color: var(--faint);
    white-space: nowrap;
  }
  .de-input {
    font: inherit;
    font-size: var(--fs-sm);
    padding: 0.22rem var(--sp-2);
    border: 1px solid var(--wire);
    border-radius: var(--r-md);
    background: var(--card);
    color: var(--ink);
    width: 100%;
    box-sizing: border-box;
  }
  .de-input:focus {
    outline: none;
    border-color: var(--primary);
  }
</style>
