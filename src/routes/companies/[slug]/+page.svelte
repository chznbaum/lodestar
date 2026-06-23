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
  import { levelLabel } from "$lib/level";
  import { fetchJobsForCompany, fetchJobDetails, cancelRun, onRunStep, onRunFinished, phaseLabel, DETAIL_STAGES, SCORING_STAGES, type DetailStage, type ScoringStage, type RunStepEvent } from "$lib/pipeline";
  import { fitBand } from "$lib/fit";
  import { sortRoles, outcomeBySlug } from "$lib/rolesView";
  import { classifyStatus, STATUS_LABELS } from "$lib/jobStatus";
  import { getCheck, listChecks } from "$lib/check";
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

  function splitList(s: string): string[] {
    return s.split(",").map((x) => x.trim()).filter(Boolean);
  }

  function sameList(a: string[], b: string[]): boolean {
    return a.length === b.length && a.every((v, i) => v === b[i]);
  }

  async function saveDetails() {
    const c = company;
    if (!c) return;

    // List fields go through the typed list command — the backend encodes them safely.
    if (!sameList(domainDraft, c.domain)) {
      await cs.setListField(c.slug, "domain", domainDraft);
    }
    const bm = splitList(detailDraft.business_model ?? "");
    if (!sameList(bm, c.business_model)) {
      await cs.setListField(c.slug, "business_model", bm);
    }

    // Scalar fields.
    const scalarKeys = ["company_size", "stage", "remote_policy", "location", "website", "careers_url", "domain_raw", "source"] as const;
    for (const key of scalarKeys) {
      const draftVal = detailDraft[key] ?? "";
      const currentVal = (c[key] ?? "") as string;
      if (draftVal !== currentVal) await cs.updateField(c.slug, key, draftVal);
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
  /** Live phase label during a run; result line when finished. Empty when idle. */
  let phase = $state("");

  // ── Triage List: per-job fetch state machine ───────────
  /** Stage status values used in the step strip. */
  type StageStatus = "done" | "now" | "failed" | "skipped";

  /**
   * Which run phase is currently active for this slug.
   * "queued"      — started in the outcome but no run:step has arrived yet.
   * "detail"      — a run:step for a DETAIL_STAGES stage has arrived.
   * "detail-done" — the detail run finished; scoring handoff has not started yet.
   *                 The Detail strip is finalized (no pulsing); Scoring strip is empty.
   *                 Transitions to "scoring" on the first SCORING_STAGES run:step.
   * "scoring"     — a run:step for a SCORING_STAGES stage has arrived.
   */
  type ActivePhase = "queued" | "detail" | "detail-done" | "scoring";

  /** A job whose detail fetch is currently running. */
  type RunStateRunning = {
    status: "running";
    runId: string;
    phase: string;
    /** Which pipeline phase is currently active. */
    activePhase: ActivePhase;
    /** Per-stage status for the Detail strip (DETAIL_STAGES). */
    detailStageStatus: Map<DetailStage, StageStatus>;
    /** Per-stage status for the Scoring strip (SCORING_STAGES). */
    scoringStageStatus: Map<ScoringStage, StageStatus>;
  };
  /** A job that was durably skipped (already running / already decided elsewhere). */
  type RunStateSkipped = { status: "skipped"; detail: string };
  /** A job that failed to start, or whose run ended in failure. */
  type RunStateFailed = {
    status: "failed";
    failedStage: string | null;
    detail: string;
  };

  type RunState = RunStateRunning | RunStateSkipped | RunStateFailed;

  /**
   * Per-slug fetch state. Populated from `fetchJobDetails` outcomes and updated live
   * by `run:step` / `run:finished` events. Running entries are removed on `run:finished`.
   */
  let runBySlug = $state<Map<string, RunState>>(new Map());

  /** Sorted roles for the Triage List (scored best-first, unscored by title). */
  let sortedJobs = $derived(sortRoles(jobs));

  /**
   * Merge a `FetchJobDetailsOutcome` onto `prev`, translating each `SlugOutcome` into
   * a `RunState`. Returns a NEW Map (so reassigning `runBySlug` triggers Svelte 5
   * reactivity — never mutate in place).
   *
   * Distinguishes failed-to-start (from the outcome `failed` bucket) from a run that
   * started then failed (handled in `run:finished`); both become `RunStateFailed` but
   * their `detail` source differs.
   */
  function applyOutcome(
    prev: Map<string, RunState>,
    outcome: Awaited<ReturnType<typeof fetchJobDetails>>,
  ): Map<string, RunState> {
    const next = new Map(prev);
    for (const [s, entry] of outcomeBySlug(outcome)) {
      if (entry.kind === "started") {
        // Seed as "queued" — the row will not animate until the first run:step arrives.
        next.set(s, {
          status: "running",
          runId: entry.runId,
          phase: "Queued…",
          activePhase: "queued",
          detailStageStatus: new Map(),
          scoringStageStatus: new Map(),
        });
      } else if (entry.kind === "skipped") {
        next.set(s, { status: "skipped", detail: entry.detail });
      } else {
        // failed-to-start
        next.set(s, { status: "failed", failedStage: null, detail: entry.detail });
      }
    }
    return next;
  }

  /** Fetch job details for the currently selected slugs. */
  async function fetchSelected() {
    if (!cs.vaultPath || selectedSlugs.length === 0) return;
    const slugsToFetch = [...selectedSlugs];
    // Clear selection immediately so the UI responds.
    selectedSlugs = [];

    let outcome: Awaited<ReturnType<typeof fetchJobDetails>>;
    try {
      outcome = await fetchJobDetails(cs.vaultPath, slugsToFetch);
    } catch (e) {
      // Failed to even start the call — mark all as failed-to-start.
      const next = new Map(runBySlug);
      for (const s of slugsToFetch) {
        next.set(s, { status: "failed", failedStage: null, detail: String(e) });
      }
      runBySlug = next;
      return;
    }

    runBySlug = applyOutcome(runBySlug, outcome);
  }

  /** Retry a single failed-to-start slug. */
  async function retrySlug(slug: string) {
    if (!cs.vaultPath) return;
    const next = new Map(runBySlug);
    next.delete(slug); // remove old failed entry so the row shows a spinner
    runBySlug = next;
    let outcome: Awaited<ReturnType<typeof fetchJobDetails>>;
    try {
      outcome = await fetchJobDetails(cs.vaultPath, [slug]);
    } catch (e) {
      const n2 = new Map(runBySlug);
      n2.set(slug, { status: "failed", failedStage: null, detail: String(e) });
      runBySlug = n2;
      return;
    }
    runBySlug = applyOutcome(runBySlug, outcome);
  }

  async function loadJobs() {
    if (!cs.vaultPath) return;
    const all = await listJobs(cs.vaultPath);
    jobs = all.filter((j) => j.company === slug);
  }
  $effect(() => {
    if (cs.vaultPath && slug) loadJobs();
  });

  // Cold-load best-effort: if a run is already in progress when the page loads (e.g. the user
  // navigated here from another page), derive the current phase from the last recorded step.
  // The live event path (onRunStep) is primary; this is secondary / best-effort.
  $effect(() => {
    if (!cs.vaultPath || running || phase) return; // already live or no vault
    const vp = cs.vaultPath;
    const currentSlug = slug;
    (async () => {
      const { listChecks } = await import("$lib/check");
      const summaries = await listChecks(vp).catch(() => []);
      for (const summary of summaries.filter((s) => s.status === "running")) {
        try {
          const full = await getCheck(vp, summary.slug);
          if (full.kind !== "job_check" || full.subject !== currentSlug) continue;
          // Found the active run — restore live state.
          runId = summary.slug;
          running = true;
          // Derive next expected stage from the chain order + last recorded step.
          const chainOrder = ["careers-scrape", "structure-listings", "finalize"];
          const lastDone = full.steps.filter((s) => s.status === "ok" || s.status === "failed").pop();
          const lastDoneIdx = lastDone ? chainOrder.indexOf(lastDone.stage) : -1;
          const nextStage = chainOrder[lastDoneIdx + 1] ?? chainOrder[0];
          phase = phaseLabel(nextStage, "running") || "Working…";
          break;
        } catch {
          // skip — don't let a bad check abort cold-load
        }
      }
    })();
  });

  // Cold-load best-effort: if any job_detail or job_scoring runs are already in progress when
  // the page loads (e.g. the user navigated away and back), seed runBySlug so the strips resume.
  // The live event path (onRunStep) is primary; this is secondary / best-effort.
  $effect(() => {
    if (!cs.vaultPath || jobs.length === 0) return;
    const vp = cs.vaultPath;
    const jobSlugs = new Set(jobs.map((j) => j.slug));
    (async () => {
      const summaries = await listChecks(vp).catch(() => []);
      const running = summaries.filter(
        (s) =>
          s.status === "running" &&
          (s.kind === "job_detail" || s.kind === "job_scoring") &&
          jobSlugs.has(s.subject),
      );
      if (running.length === 0) return;
      const next = new Map(runBySlug);
      for (const summary of running) {
        const jobSlug = summary.subject;
        // Don't overwrite an entry that is already live (seeded by applyOutcome or a prior event).
        if (next.has(jobSlug)) continue;
        // Best-effort: read the full check to derive done stages.
        let detailStageStatus = new Map<DetailStage, StageStatus>();
        let scoringStageStatus = new Map<ScoringStage, StageStatus>();
        let activePhase: ActivePhase = summary.kind === "job_scoring" ? "scoring" : "detail";
        let phaseStr = summary.kind === "job_scoring" ? "Scoring…" : "Working…";
        try {
          const full = await getCheck(vp, summary.slug);
          for (const step of full.steps) {
            if (step.status === "ok") {
              if ((DETAIL_STAGES as readonly string[]).includes(step.stage)) {
                detailStageStatus.set(step.stage as DetailStage, "done");
              } else if ((SCORING_STAGES as readonly string[]).includes(step.stage)) {
                scoringStageStatus.set(step.stage as ScoringStage, "done");
              }
            }
          }
          // If this is a scoring run, mark all detail stages complete.
          if (summary.kind === "job_scoring") {
            for (const ds of DETAIL_STAGES) {
              if (!detailStageStatus.has(ds)) detailStageStatus.set(ds, "done");
            }
          }
          // Derive best phase label from the last running/ok step.
          const lastStep = full.steps.at(-1);
          if (lastStep) {
            phaseStr = phaseLabel(lastStep.stage, "running") || phaseStr;
          }
        } catch {
          // Can't read full check — just show it as active/working.
        }
        next.set(jobSlug, {
          status: "running",
          runId: summary.slug,
          phase: phaseStr,
          activePhase,
          detailStageStatus,
          scoringStageStatus,
        });
      }
      runBySlug = next;
    })();
  });

  // Live progress: per-step + run-finished events for ALL runs on this page.
  // The discovery run is identified by `runId` (single run).
  // Job-detail/job-scoring runs for a slug are matched by `e.subject` (the slug) —
  // both the detail run and the auto-handed-off scoring run share the same slug, so
  // the strip fills 1→6 continuously across the handoff without a re-key.
  $effect(() => {
    const subs: (() => void)[] = [];
    let active = true;
    Promise.all([
      onRunStep((e: RunStepEvent) => {
        // ── Discovery run ──
        if (e.run_id === runId) {
          progress = `${e.stage}: ${e.status}`;
          const label = phaseLabel(e.stage, e.status, e.detail);
          if (label) phase = label;
          return;
        }
        // ── Job-detail/job-scoring run: match by subject (the job slug) ──
        const matchedSlug = e.subject || undefined;
        if (!matchedSlug || !runBySlug.has(matchedSlug)) return;
        const existingRs = runBySlug.get(matchedSlug);
        if (!existingRs || existingRs.status !== "running") return;

        const next = new Map(runBySlug);
        const rs = existingRs as RunStateRunning;
        const stage = e.stage;

        // Determine which strip this stage belongs to.
        const isDetailStage = (DETAIL_STAGES as readonly string[]).includes(stage);
        const isScoringStage = (SCORING_STAGES as readonly string[]).includes(stage);

        const newDetailStatus = new Map(rs.detailStageStatus);
        const newScoringStatus = new Map(rs.scoringStageStatus);
        let newActivePhase: ActivePhase = rs.activePhase;

        if (isDetailStage) {
          const ds = stage as DetailStage;
          if (e.status === "running") {
            newActivePhase = "detail";
            // Mark earlier detail stages without status as skipped.
            const stageIdx = DETAIL_STAGES.indexOf(ds);
            for (let i = 0; i < stageIdx; i++) {
              const earlier = DETAIL_STAGES[i];
              if (!newDetailStatus.has(earlier)) newDetailStatus.set(earlier, "skipped");
            }
            newDetailStatus.set(ds, "now");
          } else if (e.status === "ok") {
            newDetailStatus.set(ds, "done");
          } else if (e.status === "failed") {
            newDetailStatus.set(ds, "failed");
          }
        } else if (isScoringStage) {
          const ss = stage as ScoringStage;
          if (e.status === "running") {
            // Transition from "detail-done" (or any prior phase) to active scoring.
            // Detail strip was already finalized at detail-run:finished; no backfill needed here.
            newActivePhase = "scoring";
            // Mark earlier scoring stages without status as skipped.
            const stageIdx = SCORING_STAGES.indexOf(ss);
            for (let i = 0; i < stageIdx; i++) {
              const earlier = SCORING_STAGES[i];
              if (!newScoringStatus.has(earlier)) newScoringStatus.set(earlier, "skipped");
            }
            newScoringStatus.set(ss, "now");
          } else if (e.status === "ok") {
            newScoringStatus.set(ss, "done");
          } else if (e.status === "failed") {
            newScoringStatus.set(ss, "failed");
          }
        }
        // Unknown stage: ignore (don't crash).

        const label = phaseLabel(e.stage, e.status, e.detail);
        next.set(matchedSlug, {
          ...rs,
          // Always track the latest run_id so "cancel" hits the active run.
          runId: e.run_id,
          phase: label || rs.phase,
          activePhase: newActivePhase,
          detailStageStatus: newDetailStatus,
          scoringStageStatus: newScoringStatus,
        });
        runBySlug = next;
      }),
      onRunFinished(async (e: RunStepEvent) => {
        // ── Discovery run ──
        if (e.run_id === runId) {
          running = false;
          progress = e.status;
          await loadJobs();
          cs.load();
          if (e.status === "complete") {
            try {
              const check = cs.vaultPath ? await getCheck(cs.vaultPath, e.run_id) : null;
              const n = check?.roles_found ?? jobs.length;
              phase = `Done · ${n} new role${n === 1 ? "" : "s"}`;
            } catch {
              phase = `Done · ${jobs.length} new role${jobs.length === 1 ? "" : "s"}`;
            }
          } else if (e.status === "failed") {
            try {
              const check = cs.vaultPath ? await getCheck(cs.vaultPath, e.run_id) : null;
              const failedStep = check?.steps.find((s) => s.status === "failed");
              const reason = failedStep?.error ?? "unknown error";
              phase = `Failed · ${reason}`;
            } catch {
              phase = "Failed";
            }
          } else {
            phase = e.status;
          }
          return;
        }
        // ── Job-detail/job-scoring run: match by subject (the job slug) ──
        const matchedSlug = e.subject || undefined;
        if (!matchedSlug || !runBySlug.has(matchedSlug)) return;
        const existingRs = runBySlug.get(matchedSlug);
        if (!existingRs || existingRs.status !== "running") return;

        if (e.status === "complete") {
          // A run completed — reload to get the current job state, then decide:
          // - scored (fit_score != null)  → the scoring run finished; finalize (remove entry)
          // - detailed (fit_score == null) → the detail run just finished; finalize the Detail
          //   strip and enter "detail-done" (non-pulsing rest) while awaiting the scoring handoff.
          await loadJobs();
          const reloadedJob = jobs.find((j) => j.slug === matchedSlug);
          const next = new Map(runBySlug);
          if (!reloadedJob || reloadedJob.fit_score !== null) {
            // Scoring run finished (or job is gone) → row shows fit band.
            next.delete(matchedSlug);
          } else {
            // Detail run finished; scoring handoff is expected but hasn't started yet.
            // Finalize the Detail strip (mark any still-unset stage as "done", including a
            // skipped research-gaps → "done" which is correct and intended) and rest non-pulsing.
            const rs = existingRs as RunStateRunning;
            const finalDetailStatus = new Map(rs.detailStageStatus);
            for (const ds of DETAIL_STAGES) {
              if (!finalDetailStatus.has(ds)) finalDetailStatus.set(ds, "done");
            }
            next.set(matchedSlug, {
              ...rs,
              phase: "Detail done · awaiting scoring…",
              activePhase: "detail-done",
              detailStageStatus: finalDetailStatus,
            });
          }
          runBySlug = next;
        } else if (e.status === "failed") {
          // Run failed — build a durable failed entry with the last failed stage info.
          const rs = existingRs as RunStateRunning;
          // Check both strips for a failed stage.
          const lastFailedDetail = [...rs.detailStageStatus.entries()].find(([, v]) => v === "failed")?.[0] ?? null;
          const lastFailedScoring = [...rs.scoringStageStatus.entries()].find(([, v]) => v === "failed")?.[0] ?? null;
          const lastFailedStage = lastFailedDetail ?? lastFailedScoring;
          let detail = "Run failed";
          try {
            const check = cs.vaultPath ? await getCheck(cs.vaultPath, e.run_id) : null;
            const failedStep = check?.steps.find((s) => s.status === "failed");
            detail = failedStep?.error ?? "Run failed";
          } catch {
            // best-effort — keep generic fallback
          }
          const next = new Map(runBySlug);
          next.set(matchedSlug, {
            status: "failed",
            failedStage: lastFailedStage,
            detail,
          });
          await loadJobs();
          runBySlug = next;
        } else if (e.status === "cancelled") {
          const next = new Map(runBySlug);
          next.delete(matchedSlug);
          runBySlug = next;
          await loadJobs();
        }
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
    phase = "Starting…";
    try {
      runId = await fetchJobsForCompany(cs.vaultPath, slug);
    } catch (e) {
      progress = `error: ${e}`;
      phase = `Error: ${e}`;
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
        <span class="pipeline-phase">{phase || progress}</span>
        <button class="btn sm" onclick={cancelFetch}>Cancel</button>
      {:else}
        <button
          class="btn sm"
          onclick={fetchJobs}
          disabled={!cs.vaultPath || !c.careers_url}
          title={c.careers_url ? "Scrape this company's careers page for roles" : "No careers URL set — add one in Details"}
        >Fetch jobs</button>
        {#if phase}<span class="pipeline-phase pipeline-phase--done">{phase}</span>{/if}
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
        {#if jobs.length === 0}
          <p class="empty">{c.last_checked ? "No roles found." : "Not fetched yet — \"Fetch jobs\" lists matching roles here."}</p>
        {:else}
          <!-- sticky action bar -->
          <div class="roles__actionbar">
            <span class="roles__selcount">{selectedSlugs.length} selected</span>
            <button
              class="btn primary sm"
              disabled={selectedSlugs.length === 0 || !cs.vaultPath}
              onclick={fetchSelected}
            >Fetch selected ({selectedSlugs.length})</button>
            <button
              class="linkbtn"
              onclick={() => {
                selectedSlugs = sortedJobs
                  .filter((j) => {
                    const rs = runBySlug.get(j.slug);
                    return !rs || rs.status !== "running";
                  })
                  .map((j) => j.slug);
              }}
            >select all</button>
            <button class="linkbtn" onclick={() => (selectedSlugs = [])}>clear</button>
          </div>

          <ul class="roles__list">
            {#each sortedJobs as j (j.slug)}
              {@const rs = runBySlug.get(j.slug)}
              {@const statusDisplay = classifyStatus(j.status)}
              {@const band = fitBand(j.fit_score)}
              <li class="roles__row">
                <!-- checkbox: stop propagation so it doesn't trigger the row link -->
                <input
                  type="checkbox"
                  bind:group={selectedSlugs}
                  value={j.slug}
                  disabled={rs?.status === "running"}
                  title={rs?.status === "running" ? "a fetch for this job is already running" : undefined}
                  onclick={(ev) => ev.stopPropagation()}
                />

                <!-- fit column -->
                <div class="roles__fit">
                  {#if rs?.status === "running"}
                    <span class="roles__fitnum unscored">···</span>
                    <span class="roles__bandlbl">{rs.activePhase === "queued" ? "queued" : rs.activePhase === "scoring" ? "scoring" : rs.activePhase === "detail-done" ? "detailed" : "fetching"}</span>
                  {:else if j.fit_score !== null}
                    <span class="roles__fitnum {band.key}">{j.fit_score}</span>
                    <span class="roles__bandlbl">{band.label.toLowerCase()}</span>
                  {:else}
                    <span class="roles__fitnum unscored">—</span>
                    <span class="roles__bandlbl">new</span>
                  {/if}
                </div>

                <!-- main content column — wrapped in a link for keyboard nav + a11y -->
                <a class="roles__main" href="/jobs/{j.slug}">
                  <div class="roles__titlerow">
                    <span class="roles__title">{j.title}</span>

                    {#if rs?.status === "running"}
                      <!-- live progress indicator: two separate strips (Detail | Scoring) -->
                      <span class="roles__live">
                        <!-- dot: idle (dim, no animation) when queued or detail-done (awaiting handoff);
                             pulsing when actively fetching or scoring -->
                        <span class="roles__livedot" class:roles__livedot--idle={rs.activePhase === "queued" || rs.activePhase === "detail-done"}></span>
                        {rs.phase}
                        {#if rs.activePhase !== "queued"}
                          <!-- Detail strip (jd-scrape, structure-jd, gap-detect, research-gaps) -->
                          <span class="roles__phaselabel">Detail</span>
                          <span class="roles__steps" title={DETAIL_STAGES.join(" · ")}>
                            {#each DETAIL_STAGES as stage}
                              <i class={rs.detailStageStatus.get(stage) ?? ""}></i>
                            {/each}
                          </span>
                          <!-- Scoring strip (fit-score, alignment) -->
                          <span class="roles__phaselabel">Scoring</span>
                          <span class="roles__steps" title={SCORING_STAGES.join(" · ")}>
                            {#each SCORING_STAGES as stage}
                              <i class={rs.scoringStageStatus.get(stage) ?? ""}></i>
                            {/each}
                          </span>
                        {/if}
                      </span>
                    {:else if rs?.status === "skipped"}
                      <span class="chip flat">{rs.detail}</span>
                    {:else if rs?.status === "failed"}
                      <span class="chip danger">failed{rs.failedStage ? ` · ${rs.failedStage}` : ""}</span>
                    {:else if statusDisplay.kind === "anomaly"}
                      <span class="chip {statusDisplay.raw === null ? 'warn' : 'danger'}">{statusDisplay.message}</span>
                    {:else if j.fit_score !== null}
                      <!-- fit score is shown in the fit column; no dealbreaker chip here (no flags body available) -->
                    {:else if statusDisplay.kind === "known" && j.status === "detailed"}
                      <span class="chip flat">{STATUS_LABELS["detailed"]}</span>
                    {:else}
                      <span class="chip flat">new</span>
                    {/if}
                  </div>

                  <!-- meta: level · location -->
                  <div class="roles__meta">
                    {[levelLabel(j.level), j.location].filter(Boolean).join(" · ")}
                    {#if rs?.status === "failed" && rs.detail}
                      &nbsp;·&nbsp;<span class="sub">{rs.detail}</span>
                    {/if}
                  </div>
                </a>

                <!-- end column: actions (outside the link, stop click from bubbling to <li>) -->
                <div class="roles__end">
                  {#if rs?.status === "running"}
                    <button class="linkbtn" onclick={() => cancelRun(rs.runId)}>cancel</button>
                  {:else if rs?.status === "failed"}
                    <button class="btn sm" onclick={(ev) => { ev.stopPropagation(); retrySlug(j.slug); }}>Retry</button>
                  {:else if j.fit_score !== null || j.jd_fetched}
                    <button class="btn sm" onclick={(ev) => { ev.stopPropagation(); goto(`/jobs/${j.slug}`); }}>Open</button>
                  {/if}
                </div>
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
