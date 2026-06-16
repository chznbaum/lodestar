<script lang="ts">
  import { companiesStore as cs } from "$lib/companies.svelte";
  import type { NewCompany } from "$lib/vault";

  let { onclose, oncreated }: { onclose: () => void; oncreated: (slug: string) => void } = $props();

  let name = $state("");
  let website = $state("");
  let careers_url = $state("");
  let domain = $state(""); // comma-separated raw slugs
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
      domain: splitList(domain),
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
  <p class="hint">Raw slug values for domain/model (e.g. <code>financial_services, ai</code>). Web-research auto-fill arrives in a later phase.</p>
  {#if error}<p class="error">{error}</p>{/if}
  <label>Name<input bind:value={name} placeholder="Acme, Inc." /></label>
  <label>Website<input bind:value={website} placeholder="https://…" /></label>
  <label>Careers URL<input bind:value={careers_url} placeholder="https://…/careers" /></label>
  <label>Domain(s)<input bind:value={domain} placeholder="financial_services, ai" /></label>
  <label>Business model(s)<input bind:value={business_model} placeholder="b2b" /></label>
  <label>Size<select bind:value={company_size}>{#each SIZES as s}<option value={s}>{s || "—"}</option>{/each}</select></label>
  <label>Stage<select bind:value={stage}>{#each STAGES as s}<option value={s}>{s || "—"}</option>{/each}</select></label>
  <label>Remote<select bind:value={remote_policy}>{#each REMOTES as r}<option value={r}>{r || "—"}</option>{/each}</select></label>
  <label>Location<input bind:value={location} placeholder="Remote, US" /></label>
  <label>Notes<textarea bind:value={notes} placeholder="Why listed…"></textarea></label>
  <div class="actions">
    <button onclick={onclose}>Cancel</button>
    <button class="primary" disabled={saving} onclick={submit}>{saving ? "Creating…" : "Create"}</button>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0,0,0,.25); }
  .modal { position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(32rem, 92vw); max-height: 88vh; overflow-y: auto; background: #fff; border-radius: 12px; padding: 1.25rem 1.5rem; box-shadow: 0 12px 40px rgba(0,0,0,.2); }
  h2 { margin: 0 0 .25rem; font-size: 1.1rem; }
  .hint { color: #777; font-size: .8rem; margin: 0 0 .75rem; }
  label { display: block; font-size: .8rem; color: #555; margin-bottom: .55rem; }
  input, select, textarea { width: 100%; box-sizing: border-box; padding: .4rem .55rem; border: 1px solid #d4d4d4; border-radius: 7px; font: inherit; font-size: .88rem; margin-top: .15rem; }
  textarea { min-height: 5rem; resize: vertical; }
  .actions { display: flex; justify-content: flex-end; gap: .5rem; margin-top: .5rem; }
  .actions button { padding: .4rem .9rem; border: 1px solid #d4d4d4; border-radius: 7px; background: #fff; cursor: pointer; font: inherit; }
  .actions .primary { background: #2563eb; border-color: #2563eb; color: #fff; font-weight: 600; }
  .error { color: #b91c1c; font-size: .85rem; }
</style>
