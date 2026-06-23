<script lang="ts">
  import { page } from "$app/state";
  import { companiesStore as cs } from "$lib/companies.svelte";
  import {
    getJob,
    setJobStatus,
    updateJobField,
    setJobListField,
    type JobDetail,
    EMPLOYMENT_TYPES,
    COMP_PERIODS,
    REMOTE_KINDS,
    SPONSORSHIP,
  } from "$lib/job";
  import { jobSections, subScoreRows, hasDealbreaker } from "$lib/jobDetail";
  import { fitBand } from "$lib/fit";
  import { classifyStatus, nextHumanStatuses, STATUS_LABELS } from "$lib/jobStatus";
  import { renderMarkdown } from "$lib/markdown";
  import { levelLabel, LEVEL_LABELS } from "$lib/level";
  import { humanize } from "$lib/labels";
  import Combobox from "$lib/Combobox.svelte";
  import { onRunStep, onRunFinished, phaseLabel, rescoreJob, type RunStepEvent } from "$lib/pipeline";

  const slug = $derived(page.params.slug ?? "");

  // ---------------------------------------------------------------------------
  // Data loading
  // ---------------------------------------------------------------------------
  let job = $state<JobDetail | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(false);

  async function reload() {
    if (!cs.vaultPath || !slug) return;
    loading = true;
    loadError = null;
    try {
      job = await getJob(cs.vaultPath, slug);
    } catch (e) {
      loadError = String(e);
      job = null;
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (cs.vaultPath && !cs.loaded && !cs.loading) cs.load();
  });

  $effect(() => {
    if (cs.vaultPath && slug) reload();
  });

  // ---------------------------------------------------------------------------
  // Derived display state
  // ---------------------------------------------------------------------------
  let sections = $derived(job ? jobSections(job.body) : null);
  let band = $derived(job ? fitBand(job.fit_score) : fitBand(null));
  let scoreRows = $derived(job ? subScoreRows(job) : []);
  let statusDisplay = $derived(job ? classifyStatus(job.status) : null);
  let companySlug = $derived(job?.company ?? null);

  // Fit-flags HTML (sanitized markdown)
  let fitFlagsHtml = $derived(
    sections?.fitFlags ? renderMarkdown(sections.fitFlags) : null,
  );

  // Alignment HTML (sanitized markdown)
  let alignmentHtml = $derived(
    sections?.alignment ? renderMarkdown(sections.alignment) : null,
  );

  // JD — structured HTML (sanitized markdown)
  let jdStructuredHtml = $derived(
    sections?.jdStructured ? renderMarkdown(sections.jdStructured) : null,
  );

  // Research notes HTML (sanitized markdown)
  let researchHtml = $derived(
    sections?.research ? renderMarkdown(sections.research) : null,
  );

  // Band CSS class for coloring (maps fit band key to the appropriate token)
  function bandClass(key: string): string {
    switch (key) {
      case "strong":
      case "good":
        return "band--good";
      case "partial":
      case "weak":
        return "band--warn";
      case "mismatch":
        return "band--danger";
      default:
        return "band--unscored";
    }
  }

  // Status control options: current status + legal transitions (deduplicated, ordered)
  let statusOptions = $derived.by(() => {
    if (!job || !statusDisplay || statusDisplay.kind !== "known") return [];
    const current = statusDisplay.status;
    const next = nextHumanStatuses(current);
    // Current first, then transitions
    const allKeys = [current, ...next.filter((k) => k !== current)];
    return allKeys.map((k) => ({ value: k, label: STATUS_LABELS[k as keyof typeof STATUS_LABELS] ?? humanize(k) }));
  });

  // ---------------------------------------------------------------------------
  // Re-score on edit
  // ---------------------------------------------------------------------------
  /** "Re-scoring…" indicator text; empty when idle. */
  let rescorePhase = $state("");
  /**
   * True while a scoring run is in flight for this slug — set when:
   * (a) maybeRescore enqueues a rescore, or (b) a run:step with a SCORING stage
   * arrives for this slug (covers a background detail→scoring handoff while the
   * user is on this page). Cleared on run:finished for this slug.
   */
  let scoringInFlight = $state(false);
  /** Note shown when rescoreJob rejects unexpectedly. */
  let rescoreNote = $state("");

  // Subscribe to run:step and run:finished events for this slug — always on, torn down on teardown.
  // Indicator visibility is gated on `scoringInFlight` in the template.
  $effect(() => {
    const currentSlug = slug;
    const subs: (() => void)[] = [];
    let active = true;

    Promise.all([
      onRunStep((e: RunStepEvent) => {
        if (e.subject !== currentSlug) return;
        // A scoring stage arrived for this slug — mark in-flight (covers the
        // background detail→scoring handoff when the user is on this page).
        const isScoringStage = e.stage === "fit-score" || e.stage === "alignment";
        if (isScoringStage) scoringInFlight = true;
        if (scoringInFlight) {
          const label = phaseLabel(e.stage, e.status, e.detail);
          if (label) rescorePhase = label;
        }
      }),
      onRunFinished(async (e: RunStepEvent) => {
        if (e.subject !== currentSlug) return;
        await reload();
        scoringInFlight = false;
        rescorePhase = "";
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

  /** Trigger a rescore if the job is already scored and no scoring run is in flight. */
  async function maybeRescore() {
    // Skip if unscored (no point) or if scoring is already running (don't stack).
    if (!job || !cs.vaultPath || job.fit_score === null || scoringInFlight) return;
    rescoreNote = "";
    scoringInFlight = true;
    rescorePhase = "Re-scoring…";
    try {
      await rescoreJob(cs.vaultPath, slug);
    } catch (e) {
      // Unexpected error (the job note disappeared mid-edit, etc.) — clear the
      // flag so the UI doesn't stay stuck, and surface a quiet note.
      scoringInFlight = false;
      rescorePhase = "";
      rescoreNote = `Re-score note: ${e}`;
    }
  }

  async function handleStatusChange(newStatus: string) {
    if (!job || !cs.vaultPath) return;
    if (newStatus === job.status) return;
    try {
      await setJobStatus(cs.vaultPath, slug, newStatus);
      await reload();
    } catch (e) {
      alert(`Status change failed: ${e}`);
    }
  }

  // ---------------------------------------------------------------------------
  // Details panel — edit mode (mirrors companies pattern)
  // ---------------------------------------------------------------------------

  let editingDetails = $state(false);
  let detailDraft = $state<Record<string, string>>({});

  function openDetailEdit() {
    const j = job;
    if (!j) return;
    detailDraft = {
      comp_low: j.comp_low !== null ? String(j.comp_low) : "",
      comp_high: j.comp_high !== null ? String(j.comp_high) : "",
      comp_currency: j.comp_currency ?? "",
      comp_period: j.comp_period ?? "",
      comp_equity: j.comp_equity ?? "",
      required_skills: j.required_skills.join(", "),
      preferred_skills: j.preferred_skills.join(", "),
      remote: j.remote ?? "",
      yoe_min: j.yoe_min !== null ? String(j.yoe_min) : "",
      employment_type: j.employment_type ?? "",
      level: j.level ?? "",
      visa_sponsorship: j.visa_sponsorship ?? "",
      relocation: j.relocation ?? "",
      date_posted: j.date_posted ?? "",
      reports_to: j.reports_to ?? "",
      team: j.team ?? "",
      location_constraints: j.location_constraints ?? "",
    };
    editingDetails = true;
  }

  function cancelDetailEdit() {
    editingDetails = false;
  }

  function splitList(s: string): string[] {
    return s.split(",").map((x) => x.trim()).filter(Boolean);
  }

  function sameList(a: string[], b: string[]): boolean {
    return a.length === b.length && a.every((v, i) => v === b[i]);
  }

  async function saveDetails() {
    const j = job;
    if (!j || !cs.vaultPath) return;

    let anyChanged = false;

    // List fields
    const reqSkills = splitList(detailDraft.required_skills ?? "");
    if (!sameList(reqSkills, j.required_skills)) {
      await setJobListField(cs.vaultPath, slug, "required_skills", reqSkills);
      anyChanged = true;
    }
    const prefSkills = splitList(detailDraft.preferred_skills ?? "");
    if (!sameList(prefSkills, j.preferred_skills)) {
      await setJobListField(cs.vaultPath, slug, "preferred_skills", prefSkills);
      anyChanged = true;
    }

    // Scalar / enum fields — compare draft string to current stringified value
    const scalarComparisons: Array<{ field: string; draft: string; current: string }> = [
      { field: "comp_low", draft: detailDraft.comp_low ?? "", current: j.comp_low !== null ? String(j.comp_low) : "" },
      { field: "comp_high", draft: detailDraft.comp_high ?? "", current: j.comp_high !== null ? String(j.comp_high) : "" },
      { field: "comp_currency", draft: detailDraft.comp_currency ?? "", current: j.comp_currency ?? "" },
      { field: "comp_period", draft: detailDraft.comp_period ?? "", current: j.comp_period ?? "" },
      { field: "comp_equity", draft: detailDraft.comp_equity ?? "", current: j.comp_equity ?? "" },
      { field: "remote", draft: detailDraft.remote ?? "", current: j.remote ?? "" },
      { field: "yoe_min", draft: detailDraft.yoe_min ?? "", current: j.yoe_min !== null ? String(j.yoe_min) : "" },
      { field: "employment_type", draft: detailDraft.employment_type ?? "", current: j.employment_type ?? "" },
      { field: "level", draft: detailDraft.level ?? "", current: j.level ?? "" },
      { field: "visa_sponsorship", draft: detailDraft.visa_sponsorship ?? "", current: j.visa_sponsorship ?? "" },
      { field: "relocation", draft: detailDraft.relocation ?? "", current: j.relocation ?? "" },
      { field: "date_posted", draft: detailDraft.date_posted ?? "", current: j.date_posted ?? "" },
      { field: "reports_to", draft: detailDraft.reports_to ?? "", current: j.reports_to ?? "" },
      { field: "team", draft: detailDraft.team ?? "", current: j.team ?? "" },
      { field: "location_constraints", draft: detailDraft.location_constraints ?? "", current: j.location_constraints ?? "" },
    ];

    for (const { field, draft, current } of scalarComparisons) {
      if (draft !== current) {
        await updateJobField(cs.vaultPath, slug, field, draft);
        anyChanged = true;
      }
    }

    editingDetails = false;
    await reload();

    // Trigger re-score once — only if something changed and the job is already scored.
    if (anyChanged) {
      await maybeRescore();
    }
  }

  // ---------------------------------------------------------------------------
  // Collapsible sections
  // ---------------------------------------------------------------------------
  let detailsOpen = $state(true);
  let researchOpen = $state(true);
  let rawJdOpen = $state(false);

  // ---------------------------------------------------------------------------
  // Select options for enum fields
  // ---------------------------------------------------------------------------
  const levelOptions = Object.entries(LEVEL_LABELS).map(([v, l]) => ({ value: v, label: l }));
  const remoteOptions = REMOTE_KINDS.map((v) => ({ value: v, label: humanize(v) }));
  const employmentOptions = EMPLOYMENT_TYPES.map((v) => ({ value: v, label: humanize(v) }));
  const compPeriodOptions = COMP_PERIODS.map((v) => ({ value: v, label: humanize(v) }));
  const sponsorshipOptions = SPONSORSHIP.map((v) => ({ value: v, label: humanize(v) }));
</script>

<div class="jobs">
  <!-- breadcrumb -->
  <p class="crumbs">
    <a href="/">← Companies</a>
    {#if companySlug}
      / <a href="/companies/{companySlug}">{job?.company ?? companySlug}</a>
    {/if}
    {#if job}
      / {job.title}
    {/if}
  </p>

  {#if !cs.vaultPath}
    <p class="hint">No vault configured — open Settings to set your vault path.</p>
  {:else if loading}
    <p class="hint">Loading…</p>
  {:else if loadError}
    <p class="error">Could not load job: {loadError}</p>
    <p><a href="/">← Back to Companies</a></p>
  {:else if !job}
    <p class="hint">Job not found.</p>
    <p><a href="/">← Back to Companies</a></p>
  {:else}
    {@const j = job}

    <!-- ====================================================================
         HEADER — identity + decision controls
         ==================================================================== -->
    <header class="jobs__header">
      <!-- Score block -->
      <div class="jobs__score {bandClass(band.key)}">
        <span class="jobs__score-num">{j.fit_score !== null ? j.fit_score : "—"}</span>
        <span class="jobs__score-band">{band.label.toLowerCase()}</span>
      </div>

      <!-- Identity -->
      <div class="jobs__identity">
        <h1 class="jobs__title">{j.title}</h1>
        <p class="jobs__meta">
          {#if companySlug}
            <a href="/companies/{companySlug}">{j.company}</a>
          {:else if j.company}
            {j.company}
          {/if}
          {#if j.level}&nbsp;· {levelLabel(j.level)}{/if}
          {#if j.location}&nbsp;· {j.location}{/if}
        </p>

        <!-- Chips row: dealbreaker ([DEALBREAKER] flag in fit flags) + status anomaly -->
        <div class="jobs__chips">
          {#if hasDealbreaker(sections?.fitFlags ?? null)}
            <span class="chip danger">dealbreaker</span>
          {/if}
          {#if statusDisplay?.kind === "anomaly"}
            <span class="chip danger">Status anomaly: {statusDisplay.message}</span>
          {/if}
        </div>
      </div>

      <!-- Status control (prominent) -->
      <div class="jobs__status-ctrl">
        {#if statusDisplay?.kind === "anomaly"}
          <p class="jobs__anomaly-warn">
            Status anomaly: {statusDisplay.message}. The underlying data must be repaired before a status change is possible.
          </p>
        {:else if statusDisplay?.kind === "known"}
          {#if statusOptions.length > 1}
            <!-- has legal transitions: show combobox -->
            <Combobox
              placeholder="Status"
              value={statusDisplay.status}
              clearable={false}
              options={statusOptions}
              onchange={handleStatusChange}
            />
          {:else}
            <!-- terminal state or no transitions: read-only display -->
            <span class="chip flat">{statusDisplay.label}</span>
          {/if}
        {/if}
      </div>

      <!-- External links -->
      <div class="jobs__actions">
        {#if j.application_url}
          <a class="btn sm primary" href={j.application_url} target="_blank" rel="noreferrer">Apply</a>
        {/if}
        {#if j.url}
          <a class="btn sm" href={j.url} target="_blank" rel="noreferrer">View original posting</a>
        {/if}
      </div>
    </header>

    <!-- Re-score indicator (shown while a scoring run is in flight after an edit) -->
    {#if scoringInFlight}
      <p class="jobs__rescore-indicator">{rescorePhase || "Re-scoring…"}</p>
    {:else if rescoreNote}
      <p class="jobs__rescore-note">{rescoreNote}</p>
    {/if}

    <!-- ====================================================================
         FIT BREAKDOWN — sub-scores + fit flags
         ==================================================================== -->
    <section class="panel jobs__breakdown">
      <div class="panel__head">
        Fit breakdown
        <span class="sub">
          {j.fit_score !== null ? `${j.fit_score} / 100 · ${band.label.toLowerCase()}` : "not yet scored"}
        </span>
      </div>
      <div class="panel__body">
        {#if scoreRows.some((r) => r.value !== null)}
          <div class="jobs__subscore-list">
            {#each scoreRows as row (row.key)}
              <div class="jobs__subrow">
                <span class="jobs__sublabel">{row.label}</span>
                <div class="jobs__bar-track">
                  {#if row.value !== null}
                    <div
                      class="jobs__bar-fill {bandClass(fitBand(row.value).key)}"
                      style="width: {row.value}%"
                    ></div>
                  {/if}
                </div>
                <span class="jobs__subval {row.value !== null ? bandClass(fitBand(row.value).key) : 'band--unscored'}">
                  {row.value !== null ? row.value : "—"}
                </span>
              </div>
            {/each}
          </div>
        {:else}
          <p class="empty">Sub-scores not yet available — run "Fetch selected" from the Roles tab first.</p>
        {/if}

        {#if fitFlagsHtml}
          <div class="jobs__fitflags">
            <p class="jobs__fitflags-head">Fit flags</p>
            <!-- sanitized renderMarkdown output — LLM-generated but DOMPurify-cleaned -->
            <div class="notes">{@html fitFlagsHtml}</div>
          </div>
        {/if}
      </div>
    </section>

    <!-- ====================================================================
         HERO — Alignment analysis (narrative lead)
         ==================================================================== -->
    <section class="panel jobs__hero">
      <div class="panel__head">Alignment analysis <span class="sub">qualitative assessment</span></div>
      <div class="panel__body">
        {#if alignmentHtml}
          <!-- sanitized renderMarkdown output — LLM-generated but DOMPurify-cleaned -->
          <div class="notes">{@html alignmentHtml}</div>
        {:else}
          <p class="empty">No alignment analysis yet — run "Fetch selected" to score this role.</p>
        {/if}
      </div>
    </section>

    <!-- ====================================================================
         DETAILS — editable fields + JD structured brief (expanded by default)
         ==================================================================== -->
    <section class="panel jobs__section">
      <div class="panel__head">
        <span>
          <button class="linkbtn" onclick={() => (detailsOpen = !detailsOpen)}>
            {detailsOpen ? "hide" : "show"}
          </button>
          Details
          {#if j.researched.length > 0}
            <span class="sub">{j.researched.length} field{j.researched.length === 1 ? "" : "s"} researched</span>
          {/if}
        </span>
        {#if detailsOpen}
          {#if editingDetails}
            <span>
              <button class="linkbtn" onclick={cancelDetailEdit}>cancel</button>
              <button class="btn sm" onclick={saveDetails}>Save</button>
            </span>
          {:else}
            <button class="linkbtn" onclick={openDetailEdit}>edit</button>
          {/if}
        {/if}
      </div>

      {#if detailsOpen}
        <div class="panel__body">
          {#if editingDetails}
            <!-- Edit mode: all fields as inputs/selects -->
            <dl class="meta-grid">

              <dt>Comp low</dt>
              <dd><input type="text" bind:value={detailDraft.comp_low} /></dd>

              <dt>Comp high</dt>
              <dd><input type="text" bind:value={detailDraft.comp_high} /></dd>

              <dt>Currency</dt>
              <dd><input type="text" bind:value={detailDraft.comp_currency} /></dd>

              <dt>Comp period</dt>
              <dd>
                <select bind:value={detailDraft.comp_period}>
                  <option value="">—</option>
                  {#each compPeriodOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>Equity</dt>
              <dd><input type="text" bind:value={detailDraft.comp_equity} /></dd>

              <dt>Required skills</dt>
              <dd><input type="text" bind:value={detailDraft.required_skills} placeholder="comma-separated" /></dd>

              <dt>Preferred skills</dt>
              <dd><input type="text" bind:value={detailDraft.preferred_skills} placeholder="comma-separated" /></dd>

              <dt>Remote</dt>
              <dd>
                <select bind:value={detailDraft.remote}>
                  <option value="">—</option>
                  {#each remoteOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>YOE min</dt>
              <dd><input type="text" bind:value={detailDraft.yoe_min} /></dd>

              <dt>Employment</dt>
              <dd>
                <select bind:value={detailDraft.employment_type}>
                  <option value="">—</option>
                  {#each employmentOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>Level</dt>
              <dd>
                <select bind:value={detailDraft.level}>
                  <option value="">—</option>
                  {#each levelOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>Visa sponsorship</dt>
              <dd>
                <select bind:value={detailDraft.visa_sponsorship}>
                  <option value="">—</option>
                  {#each sponsorshipOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>Relocation</dt>
              <dd>
                <select bind:value={detailDraft.relocation}>
                  <option value="">—</option>
                  {#each sponsorshipOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              </dd>

              <dt>Posted</dt>
              <dd><input type="text" bind:value={detailDraft.date_posted} /></dd>

              <dt>Reports to</dt>
              <dd><input type="text" bind:value={detailDraft.reports_to} /></dd>

              <dt>Team</dt>
              <dd><input type="text" bind:value={detailDraft.team} /></dd>

              <dt>Location notes</dt>
              <dd><input type="text" bind:value={detailDraft.location_constraints} /></dd>

            </dl>
          {:else}
            <!-- Read mode -->
            <dl class="meta-grid">

              <dt>Comp low</dt>
              <dd>
                {j.comp_low !== null ? j.comp_low.toLocaleString() : "—"}
                {#if j.researched.includes("comp_low")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Comp high</dt>
              <dd>
                {j.comp_high !== null ? j.comp_high.toLocaleString() : "—"}
                {#if j.researched.includes("comp_high")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Currency</dt>
              <dd>
                {j.comp_currency ?? "—"}
                {#if j.researched.includes("comp_currency")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Comp period</dt>
              <dd>
                {j.comp_period ? humanize(j.comp_period) : "—"}
                {#if j.researched.includes("comp_period")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Equity</dt>
              <dd>
                {j.comp_equity ?? "—"}
                {#if j.researched.includes("comp_equity")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Required skills</dt>
              <dd>
                {j.required_skills.length > 0 ? j.required_skills.join(", ") : "—"}
                {#if j.researched.includes("required_skills")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Preferred skills</dt>
              <dd>
                {j.preferred_skills.length > 0 ? j.preferred_skills.join(", ") : "—"}
                {#if j.researched.includes("preferred_skills")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Remote</dt>
              <dd>
                {j.remote ? humanize(j.remote) : "—"}
                {#if j.researched.includes("remote")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>YOE min</dt>
              <dd>
                {j.yoe_min !== null ? `${j.yoe_min}+` : "—"}
                {#if j.researched.includes("yoe_min")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Employment</dt>
              <dd>
                {j.employment_type ? humanize(j.employment_type) : "—"}
                {#if j.researched.includes("employment_type")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Level</dt>
              <dd>
                {j.level ? levelLabel(j.level) : "—"}
                {#if j.researched.includes("level")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Visa sponsorship</dt>
              <dd>
                {j.visa_sponsorship ? humanize(j.visa_sponsorship) : "—"}
                {#if j.researched.includes("visa_sponsorship")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Relocation</dt>
              <dd>
                {j.relocation ? humanize(j.relocation) : "—"}
                {#if j.researched.includes("relocation")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

              <dt>Posted</dt>
              <dd>{j.date_posted ?? "—"}</dd>

              <dt>Reports to</dt>
              <dd>{j.reports_to ?? "—"}</dd>

              <dt>Team</dt>
              <dd>{j.team ?? "—"}</dd>

              <dt>Location notes</dt>
              <dd>
                {j.location_constraints ?? "—"}
                {#if j.researched.includes("location_constraints")} <span class="jobs__researched">(researched)</span>{/if}
              </dd>

            </dl>
          {/if}

          {#if jdStructuredHtml}
            <div class="jobs__jd-brief">
              <!-- sanitized renderMarkdown output — LLM-generated but DOMPurify-cleaned -->
              <div class="notes">{@html jdStructuredHtml}</div>
            </div>
          {/if}
        </div>
      {/if}
    </section>

    <!-- ====================================================================
         RAW JD REFERENCE (collapsible) — path + link, never @html
         ==================================================================== -->
    <section class="panel jobs__section">
      <div class="panel__head">
        <span>
          <button class="linkbtn" onclick={() => (rawJdOpen = !rawJdOpen)}>
            {rawJdOpen ? "hide" : "show"}
          </button>
          Raw JD
          {#if j.jd_raw_file}
            <span class="sub">{j.jd_raw_file}</span>
          {/if}
        </span>
      </div>

      {#if rawJdOpen}
        <div class="panel__body">
          {#if j.jd_raw_file}
            <p class="jobs__rawpath">{j.jd_raw_file}</p>
          {/if}
          {#if j.url}
            <p>
              <a href={j.url} target="_blank" rel="noreferrer">View original posting</a>
            </p>
          {:else}
            <p class="empty">No original URL recorded.</p>
          {/if}
        </div>
      {/if}
    </section>

    <!-- ====================================================================
         RESEARCH NOTES (expanded by default)
         ==================================================================== -->
    {#if sections?.research}
      <section class="panel jobs__section">
        <div class="panel__head">
          <span>
            <button class="linkbtn" onclick={() => (researchOpen = !researchOpen)}>
              {researchOpen ? "hide" : "show"}
            </button>
            Research notes
          </span>
        </div>

        {#if researchOpen}
          <div class="panel__body">
            <!-- sanitized renderMarkdown output — LLM-generated but DOMPurify-cleaned -->
            <div class="notes">{@html researchHtml}</div>
          </div>
        {/if}
      </section>
    {/if}

  {/if}
</div>
