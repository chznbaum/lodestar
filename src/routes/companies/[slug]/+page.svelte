<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import { domainsStore as ds } from "$lib/domains.svelte";
  import { COMPANY_STATUSES } from "$lib/company";
  import { humanize, humanizeList } from "$lib/labels";
  import { renderNotes } from "$lib/markdown";
  import Combobox from "$lib/Combobox.svelte";
  import DomainPicker from "$lib/DomainPicker.svelte";
  import { listJobs, type Job } from "$lib/job";
  import { fetchJobsForCompany, cancelRun, onRunStep, onRunFinished } from "$lib/pipeline";
  import { onRecordChanged } from "$lib/vaultSync";

  const slug = $derived(page.params.slug ?? "");
  const company = $derived(cs.bySlug(slug));
  const domainResolved = $derived(
    company ? ds.resolve(company.domain) : { names: [] as string[], unknown: [] as string[] },
  );

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
    if (cs.vaultPath && !ds.loadedFor(cs.vaultPath)) ds.load(cs.vaultPath);
  });

  let editingNotes = $state(false);
  let notesDraft = $state("");
  $effect(() => { if (!editingNotes) notesDraft = company?.notes ?? ""; });

  let editingDetails = $state(false);
  let detailDraft = $state<Record<string, string>>({});
  let domainDraft = $state<string[]>([]);

  function openDetailEdit() {
    const c = company;
    if (!c) return;
    domainDraft = [...c.domain];
    detailDraft = {
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

    // domain — from the validated picker (string[])
    if (currentFmtList(domainDraft) !== currentFmtList(c.domain)) {
      writes.push(["domain", currentFmtList(domainDraft)]);
    }

    // business_model — free-text list field
    const bmFormatted = fmtList(detailDraft.business_model ?? "");
    if (bmFormatted !== currentFmtList(c.business_model)) {
      writes.push(["business_model", bmFormatted]);
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

  // ── Discovery: fetch + the selection gate ─────────────
  let jobs = $state<Job[]>([]);
  let selectedSlugs = $state<string[]>([]);
  let runId = $state<string | null>(null);
  let running = $state(false);
  let progress = $state("");

  async function loadJobs() {
    if (!cs.vaultPath) return;
    const all = await listJobs(cs.vaultPath);
    jobs = all.filter((j) => j.company === slug);
  }
  $effect(() => {
    if (cs.vaultPath && slug) loadJobs();
  });

  // Live progress: per-step + run-finished events for the run we started.
  $effect(() => {
    const subs: (() => void)[] = [];
    let active = true;
    Promise.all([
      onRunStep((e) => {
        if (e.run_id === runId) progress = `${e.stage}: ${e.status}`;
      }),
      onRunFinished((e) => {
        if (e.run_id !== runId) return;
        running = false;
        progress = e.status;
        loadJobs();
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

  // External edits to job notes (e.g. in Obsidian) refresh this company's list. Company-note
  // edits need no handler here — companiesStore reloads centrally (vaultSync) and `company`
  // derives from it. Run-driven job writes arrive via run:finished above, not the watcher.
  $effect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;
    onRecordChanged((e) => {
      if (e.kind === "job") loadJobs();
    }).then((fn) => (active ? (unlisten = fn) : fn()));
    return () => {
      active = false;
      unlisten?.();
    };
  });

  async function fetchJobs() {
    if (!cs.vaultPath || running) return;
    running = true;
    progress = "starting…";
    try {
      runId = await fetchJobsForCompany(cs.vaultPath, slug);
    } catch (e) {
      progress = `error: ${e}`;
      running = false;
    }
  }
  async function cancelFetch() {
    if (runId) await cancelRun(runId);
  }
</script>

<div class="workspace">
  {#if !company}
    <p class="crumbs"><a href="/">← Companies</a></p>
    <p class="hint">{cs.loading ? "Loading…" : "Company not found."}</p>
  {:else}
    {@const c = company}

    <p class="crumbs"><a href="/">← Companies</a> / {c.name}</p>

    <header class="workspace__header">
      <div class="workspace__name-block">
        <h1 class="workspace__name">{c.name}</h1>
        <p class="workspace__sub">{[domainResolved.names.join(", "), humanize(c.company_size ?? ""), humanize(c.stage ?? ""), humanize(c.remote_policy ?? ""), c.location].filter(Boolean).join(" · ")}</p>
      </div>
      {#if c.screening}
        {#if c.screening === "dealbreaker"}
          <span class="chip danger">{c.screening}</span>
        {:else}
          <span class="chip warn">{c.screening}</span>
        {/if}
      {:else}
        <span class="chip good">no flags</span>
      {/if}
      <div class="workspace__actions">
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
      {#if running}
        <span class="sub">⟳ {progress}</span>
        <button class="btn sm" onclick={cancelFetch}>Cancel</button>
      {:else}
        <button
          class="btn sm"
          onclick={fetchJobs}
          disabled={!cs.vaultPath || !c.careers_url}
          title={c.careers_url ? "Scrape this company's careers page for roles" : "No careers URL set — add one in Details"}
        >Fetch jobs</button>
      {/if}
    </div>

    <div class="tabs">
      <button class="tab" class:on={wtab === "overview"} onclick={() => (wtab = "overview")}>Overview</button>
      <button class="tab" class:on={wtab === "roles"} onclick={() => (wtab = "roles")}>Roles</button>
      <button class="tab" class:on={wtab === "activity"} onclick={() => (wtab = "activity")}>Activity</button>
    </div>

    {#if wtab === "overview"}
      <section class="panel">
        <div class="panel__head">Roles found <span class="sub">{jobs.length}</span></div>
        {#if jobs.length === 0}
          <p class="empty">{c.last_checked ? "No roles found." : "Not fetched yet — \"Fetch jobs\" lists matching roles here."}</p>
        {:else}
          <p class="empty">{jobs.length} role{jobs.length === 1 ? "" : "s"} — open the <button class="linkbtn" onclick={() => (wtab = "roles")}>Roles</button> tab to select which to deep-fetch.</p>
        {/if}
      </section>

      <section class="panel">
        <div class="panel__head">
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
        <div class="panel__head">
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
          <div class="panel__body">
            <div class="detail-field">
              <span class="detail-field__label">Domain</span>
              <DomainPicker bind:value={domainDraft} />

              <label class="detail-field__label" for="de-business_model">Model</label>
              <input id="de-business_model" type="text" bind:value={detailDraft.business_model} placeholder="e.g. saas, marketplace" />

              <label class="detail-field__label" for="de-company_size">Size</label>
              <Combobox id="de-company_size" placeholder="Size" bind:value={detailDraft.company_size} options={sizeOptions} />
              <label class="detail-field__label" for="de-stage">Stage</label>
              <Combobox id="de-stage" placeholder="Stage" bind:value={detailDraft.stage} options={stageOptions} />
              <label class="detail-field__label" for="de-remote_policy">Remote</label>
              <Combobox id="de-remote_policy" placeholder="Remote" bind:value={detailDraft.remote_policy} options={remoteOptions} />

              <label class="detail-field__label" for="de-location">Location</label>
              <input id="de-location" type="text" bind:value={detailDraft.location} />

              <label class="detail-field__label" for="de-website">Website</label>
              <input id="de-website" type="text" bind:value={detailDraft.website} />

              <label class="detail-field__label" for="de-careers_url">Careers</label>
              <input id="de-careers_url" type="text" bind:value={detailDraft.careers_url} />

              <label class="detail-field__label" for="de-domain_raw">Domain (raw)</label>
              <input id="de-domain_raw" type="text" bind:value={detailDraft.domain_raw} />

              <label class="detail-field__label" for="de-source">Source</label>
              <input id="de-source" type="text" bind:value={detailDraft.source} />
            </div>
          </div>
        {:else}
          <div class="panel__body">
            <dl class="meta-grid">
              <dt>Domain</dt><dd>{domainResolved.names.join(", ") || "—"}{#if c.domain_raw}&nbsp;<span class="sub-sm">({c.domain_raw})</span>{/if}{#if domainResolved.unknown.length}<span class="domain-error">⚠ unknown domain: {domainResolved.unknown.join(", ")} — no note in jobsearch-vault/domains/</span>{/if}</dd>
              <dt>Model</dt><dd>{humanizeList(c.business_model) || "—"}</dd>
              <dt>Size</dt><dd>{humanize(c.company_size ?? "") || "—"}</dd>
              <dt>Stage</dt><dd>{humanize(c.stage ?? "") || "—"}</dd>
              <dt>Remote</dt><dd>{humanize(c.remote_policy ?? "") || "—"}</dd>
              <dt>Location</dt><dd>{c.location ?? "—"}</dd>
              <dt>Website</dt><dd class="mono">{c.website ?? "—"}</dd>
              <dt>Careers</dt><dd class="mono">{c.careers_url ?? "—"}</dd>
              <dt>Source</dt><dd class="mono">{c.source ?? "—"}</dd>
            </dl>
          </div>
        {/if}
      </section>
    {/if}

    {#if wtab === "roles"}
      <section class="panel">
        <div class="panel__head">
          <span>Roles found <span class="sub">{jobs.length}</span></span>
          {#if jobs.length}
            <span class="roles__actions">
              <button class="linkbtn" onclick={() => (selectedSlugs = jobs.map((j) => j.slug))}>select all</button>
              <button class="linkbtn" onclick={() => (selectedSlugs = [])}>clear</button>
              <button class="btn sm" disabled title="JD fetch arrives in Phase B">Fetch selected ({selectedSlugs.length})</button>
            </span>
          {/if}
        </div>
        {#if jobs.length === 0}
          <p class="empty">{c.last_checked ? "No roles found." : "Not fetched yet — \"Fetch jobs\" lists matching roles here."}</p>
        {:else}
          <ul class="roles">
            {#each jobs as j (j.slug)}
              <li class="roles__row">
                <input type="checkbox" bind:group={selectedSlugs} value={j.slug} />
                <span class="roles__title">{j.title}</span>
                <span class="roles__meta">{[humanize(j.classification ?? ""), j.location].filter(Boolean).join(" · ")}</span>
                <span class="chip flat">{j.jd_fetched ? "fetched" : "new"}</span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/if}

    {#if wtab === "activity"}
      <section class="panel">
        <div class="panel__head">Activity</div>
        <p class="empty">Check history appears here once the pipeline runs (Phase 3+).</p>
      </section>
    {/if}
  {/if}
</div>
