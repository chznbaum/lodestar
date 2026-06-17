<script lang="ts">
  import { companiesStore as cs } from "$lib/companies.svelte";
  import type { NewCompany } from "$lib/vault";
  import Combobox from "$lib/Combobox.svelte";
  import DomainPicker from "$lib/DomainPicker.svelte";
  import { humanize } from "$lib/labels";

  let { onclose, oncreated }: { onclose: () => void; oncreated: (slug: string) => void } = $props();

  let name = $state("");
  let website = $state("");
  let careers_url = $state("");
  let domain = $state<string[]>([]);
  let business_model = $state("");
  let company_size = $state("");
  let stage = $state("");
  let remote_policy = $state("");
  let location = $state("");
  let notes = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);

  const SIZES = ["", "startup", "scaleup", "mid_market", "enterprise"];
  const STAGES = ["", "pre_seed", "seed", "series_a", "series_b", "series_c_plus", "public", "bootstrapped", "unknown"];
  const REMOTES = ["", "fully_remote", "remote_first", "hybrid", "onsite", "unknown"];

  const sizeOpts = SIZES.filter(Boolean).map((s) => ({ label: humanize(s), value: s }));
  const stageOpts = STAGES.filter(Boolean).map((s) => ({ label: humanize(s), value: s }));
  const remoteOpts = REMOTES.filter(Boolean).map((r) => ({ label: humanize(r), value: r }));

  function splitList(s: string): string[] {
    return s.split(",").map((x) => x.trim()).filter(Boolean);
  }

  async function submit() {
    error = null;
    if (!name.trim()) { error = "Name is required."; return; }
    saving = true;
    const payload: NewCompany = {
      name: name.trim(),
      website: website.trim() || null,
      careers_url: careers_url.trim() || null,
      domain: domain,
      business_model: splitList(business_model),
      domain_raw: null,
      company_size: company_size || null,
      stage: stage || null,
      remote_policy: remote_policy || null,
      location: location.trim() || null,
      source: "manual",
      notes: notes.trim(),
    };
    try {
      const c = await cs.create(payload);
      oncreated(c.slug);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape") onclose(); }} />
<div class="backdrop" onclick={onclose} role="presentation"></div>
<div class="modal" role="dialog" aria-modal="true" aria-label="Add company">
  <h2>Add company</h2>
  <p class="hint">Pick domains from the list. Business model is a raw slug (e.g. <code>b2b</code>). Web-research auto-fill arrives in a later phase.</p>
  {#if error}<p class="error">{error}</p>{/if}
  <label>Name<input bind:value={name} placeholder="Acme, Inc." /></label>
  <label>Website<input bind:value={website} placeholder="https://…" /></label>
  <label>Careers URL<input bind:value={careers_url} placeholder="https://…/careers" /></label>
  <div class="field"><span class="flabel">Domain(s)</span><DomainPicker bind:value={domain} /></div>
  <label>Business model(s)<input bind:value={business_model} placeholder="b2b" /></label>
  <div class="field"><span class="flabel">Size</span><Combobox placeholder="Size" bind:value={company_size} options={sizeOpts} /></div>
  <div class="field"><span class="flabel">Stage</span><Combobox placeholder="Stage" bind:value={stage} options={stageOpts} /></div>
  <div class="field"><span class="flabel">Remote</span><Combobox placeholder="Remote" bind:value={remote_policy} options={remoteOpts} /></div>
  <label>Location<input bind:value={location} placeholder="Remote, US" /></label>
  <label>Notes<textarea bind:value={notes} placeholder="Why listed…"></textarea></label>
  <div class="actions">
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={saving} onclick={submit}>{saving ? "Creating…" : "Create"}</button>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0,0,0,.25); }
  .modal { position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(32rem, 92vw); max-height: 88vh; overflow-y: auto; background: var(--card); border-radius: var(--r-lg); padding: var(--sp-5) var(--sp-5); box-shadow: 0 12px 40px rgba(0,0,0,.2); }
  h2 { margin: 0 0 var(--sp-1); font-size: var(--fs-lg); }
  .hint { margin: 0 0 var(--sp-3); }
  label { display: block; font-size: var(--fs-sm); color: var(--muted); margin-bottom: .55rem; }
  input, textarea { width: 100%; box-sizing: border-box; padding: .4rem .55rem; border: 1px solid var(--wire); border-radius: var(--r-md); font: inherit; font-size: var(--fs-md); color: var(--ink); background: var(--card); margin-top: .15rem; }
  textarea { min-height: 5rem; resize: vertical; }
  .actions { display: flex; justify-content: flex-end; gap: var(--sp-2); margin-top: var(--sp-2); }
  .field { display: block; margin-bottom: 0.55rem; }
  .flabel { display: block; font-size: var(--fs-sm); color: var(--muted); margin-bottom: 0.15rem; }
</style>
