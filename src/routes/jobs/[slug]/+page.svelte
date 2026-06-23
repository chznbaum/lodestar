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
  import { jobSections, subScoreRows } from "$lib/jobDetail";
  import { fitBand } from "$lib/fit";
  import { classifyStatus, nextHumanStatuses, STATUS_LABELS } from "$lib/jobStatus";
  import { renderMarkdown } from "$lib/markdown";
  import { levelLabel, LEVEL_LABELS } from "$lib/level";
  import { humanize } from "$lib/labels";
  import Combobox from "$lib/Combobox.svelte";

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
  // Inline field editing
  // ---------------------------------------------------------------------------

  // Active edit: { field, value } — only one field editable at a time
  let editField = $state<string | null>(null);
  let editValue = $state<string>("");

  function startEdit(field: string, current: string) {
    editField = field;
    editValue = current;
  }

  function cancelEdit() {
    editField = null;
    editValue = "";
  }

  async function commitScalar(field: string) {
    if (!cs.vaultPath || editField !== field) return;
    const val = editValue.trim();
    editField = null;
    editValue = "";
    try {
      await updateJobField(cs.vaultPath, slug, field, val);
      await reload();
    } catch (e) {
      alert(`Save failed for ${field}: ${e}`);
    }
  }

  async function commitEnum(field: string, value: string) {
    if (!cs.vaultPath) return;
    editField = null;
    editValue = "";
    try {
      await updateJobField(cs.vaultPath, slug, field, value);
      await reload();
    } catch (e) {
      alert(`Save failed for ${field}: ${e}`);
    }
  }

  async function commitList(field: string) {
    if (!cs.vaultPath || editField !== field) return;
    const values = editValue
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    editField = null;
    editValue = "";
    try {
      await setJobListField(cs.vaultPath, slug, field, values);
      await reload();
    } catch (e) {
      alert(`Save failed for ${field}: ${e}`);
    }
  }

  function onKeydown(e: KeyboardEvent, commitFn: () => void) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitFn();
    } else if (e.key === "Escape") {
      cancelEdit();
    }
  }

  // ---------------------------------------------------------------------------
  // Collapsible sections
  // ---------------------------------------------------------------------------
  let jdOpen = $state(false);
  let researchOpen = $state(false);
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

        <!-- Chips row: dealbreaker (score 0) + status anomaly -->
        <div class="jobs__chips">
          {#if j.fit_score === 0}
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
         STRUCTURED JD FIELDS (collapsible)
         ==================================================================== -->
    <section class="panel jobs__section">
      <div class="panel__head">
        <span>
          <button class="linkbtn" onclick={() => (jdOpen = !jdOpen)}>
            {jdOpen ? "hide" : "show"}
          </button>
          Structured JD
          {#if j.researched.length > 0}
            <span class="sub">{j.researched.length} field{j.researched.length === 1 ? "" : "s"} researched</span>
          {/if}
        </span>
      </div>

      {#if jdOpen}
        <div class="panel__body">
          <dl class="meta-grid">

            <!-- Comp low -->
            <dt>Comp low</dt>
            <dd>
              {#if editField === "comp_low"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("comp_low")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("comp_low"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("comp_low", String(j.comp_low ?? ""))}>
                  {j.comp_low !== null ? j.comp_low.toLocaleString() : "—"}
                </button>
                {#if j.researched.includes("comp_low")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Comp high -->
            <dt>Comp high</dt>
            <dd>
              {#if editField === "comp_high"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("comp_high")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("comp_high"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("comp_high", String(j.comp_high ?? ""))}>
                  {j.comp_high !== null ? j.comp_high.toLocaleString() : "—"}
                </button>
                {#if j.researched.includes("comp_high")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Comp currency -->
            <dt>Currency</dt>
            <dd>
              {#if editField === "comp_currency"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("comp_currency")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("comp_currency"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("comp_currency", j.comp_currency ?? "")}>
                  {j.comp_currency ?? "—"}
                </button>
                {#if j.researched.includes("comp_currency")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Comp period (enum) -->
            <dt>Comp period</dt>
            <dd>
              {#if editField === "comp_period"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("comp_period", editValue)}
                  onblur={() => commitEnum("comp_period", editValue)}
                >
                  <option value="">—</option>
                  {#each compPeriodOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("comp_period", j.comp_period ?? "")}>
                  {j.comp_period ? humanize(j.comp_period) : "—"}
                </button>
                {#if j.researched.includes("comp_period")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Comp equity -->
            <dt>Equity</dt>
            <dd>
              {#if editField === "comp_equity"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("comp_equity")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("comp_equity"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("comp_equity", j.comp_equity ?? "")}>
                  {j.comp_equity ?? "—"}
                </button>
                {#if j.researched.includes("comp_equity")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Required skills (list) -->
            <dt>Required skills</dt>
            <dd>
              {#if editField === "required_skills"}
                <input
                  type="text"
                  bind:value={editValue}
                  placeholder="comma-separated"
                  onblur={() => commitList("required_skills")}
                  onkeydown={(e) => onKeydown(e, () => commitList("required_skills"))}
                />
              {:else}
                <button
                  class="jobs__field-val linkbtn"
                  onclick={() => startEdit("required_skills", j.required_skills.join(", "))}
                >
                  {j.required_skills.length > 0
                    ? j.required_skills.map((s) => s).join(", ")
                    : "—"}
                </button>
                {#if j.researched.includes("required_skills")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Preferred skills (list) -->
            <dt>Preferred skills</dt>
            <dd>
              {#if editField === "preferred_skills"}
                <input
                  type="text"
                  bind:value={editValue}
                  placeholder="comma-separated"
                  onblur={() => commitList("preferred_skills")}
                  onkeydown={(e) => onKeydown(e, () => commitList("preferred_skills"))}
                />
              {:else}
                <button
                  class="jobs__field-val linkbtn"
                  onclick={() => startEdit("preferred_skills", j.preferred_skills.join(", "))}
                >
                  {j.preferred_skills.length > 0
                    ? j.preferred_skills.map((s) => s).join(", ")
                    : "—"}
                </button>
                {#if j.researched.includes("preferred_skills")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Remote (enum) -->
            <dt>Remote</dt>
            <dd>
              {#if editField === "remote"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("remote", editValue)}
                  onblur={() => commitEnum("remote", editValue)}
                >
                  <option value="">—</option>
                  {#each remoteOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("remote", j.remote ?? "")}>
                  {j.remote ? humanize(j.remote) : "—"}
                </button>
                {#if j.researched.includes("remote")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- YOE min -->
            <dt>YOE min</dt>
            <dd>
              {#if editField === "yoe_min"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("yoe_min")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("yoe_min"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("yoe_min", String(j.yoe_min ?? ""))}>
                  {j.yoe_min !== null ? `${j.yoe_min}+` : "—"}
                </button>
                {#if j.researched.includes("yoe_min")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Employment type (enum) -->
            <dt>Employment</dt>
            <dd>
              {#if editField === "employment_type"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("employment_type", editValue)}
                  onblur={() => commitEnum("employment_type", editValue)}
                >
                  <option value="">—</option>
                  {#each employmentOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("employment_type", j.employment_type ?? "")}>
                  {j.employment_type ? humanize(j.employment_type) : "—"}
                </button>
                {#if j.researched.includes("employment_type")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Level (enum) -->
            <dt>Level</dt>
            <dd>
              {#if editField === "level"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("level", editValue)}
                  onblur={() => commitEnum("level", editValue)}
                >
                  <option value="">—</option>
                  {#each levelOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("level", j.level ?? "")}>
                  {j.level ? levelLabel(j.level) : "—"}
                </button>
                {#if j.researched.includes("level")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Visa sponsorship (enum) -->
            <dt>Visa sponsorship</dt>
            <dd>
              {#if editField === "visa_sponsorship"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("visa_sponsorship", editValue)}
                  onblur={() => commitEnum("visa_sponsorship", editValue)}
                >
                  <option value="">—</option>
                  {#each sponsorshipOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("visa_sponsorship", j.visa_sponsorship ?? "")}>
                  {j.visa_sponsorship ? humanize(j.visa_sponsorship) : "—"}
                </button>
                {#if j.researched.includes("visa_sponsorship")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Relocation (enum) -->
            <dt>Relocation</dt>
            <dd>
              {#if editField === "relocation"}
                <select
                  bind:value={editValue}
                  onchange={() => commitEnum("relocation", editValue)}
                  onblur={() => commitEnum("relocation", editValue)}
                >
                  <option value="">—</option>
                  {#each sponsorshipOptions as opt (opt.value)}
                    <option value={opt.value}>{opt.label}</option>
                  {/each}
                </select>
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("relocation", j.relocation ?? "")}>
                  {j.relocation ? humanize(j.relocation) : "—"}
                </button>
                {#if j.researched.includes("relocation")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

            <!-- Date posted -->
            <dt>Posted</dt>
            <dd>
              {#if editField === "date_posted"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("date_posted")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("date_posted"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("date_posted", j.date_posted ?? "")}>
                  {j.date_posted ?? "—"}
                </button>
              {/if}
            </dd>

            <!-- Reports to -->
            <dt>Reports to</dt>
            <dd>
              {#if editField === "reports_to"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("reports_to")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("reports_to"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("reports_to", j.reports_to ?? "")}>
                  {j.reports_to ?? "—"}
                </button>
              {/if}
            </dd>

            <!-- Team -->
            <dt>Team</dt>
            <dd>
              {#if editField === "team"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("team")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("team"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("team", j.team ?? "")}>
                  {j.team ?? "—"}
                </button>
              {/if}
            </dd>

            <!-- Location constraints -->
            <dt>Location notes</dt>
            <dd>
              {#if editField === "location_constraints"}
                <input
                  type="text"
                  bind:value={editValue}
                  onblur={() => commitScalar("location_constraints")}
                  onkeydown={(e) => onKeydown(e, () => commitScalar("location_constraints"))}
                />
              {:else}
                <button class="jobs__field-val linkbtn" onclick={() => startEdit("location_constraints", j.location_constraints ?? "")}>
                  {j.location_constraints ?? "—"}
                </button>
                {#if j.researched.includes("location_constraints")} <span class="jobs__researched">(researched)</span>{/if}
              {/if}
            </dd>

          </dl>

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
         RESEARCH NOTES (collapsible)
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
