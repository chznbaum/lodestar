<script lang="ts">
  import { companiesStore as cs } from "$lib/companies.svelte";
  import type { NewCompany } from "$lib/company";
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
<div class="scrim" onclick={onclose} role="presentation"></div>
<div class="modal create-form" role="dialog" aria-modal="true" aria-label="Add company">
  <h2>Add company</h2>
  <p class="hint">Pick domains from the list. Business model is a raw slug (e.g. <code>b2b</code>). Web-research auto-fill arrives in a later phase.</p>
  {#if error}<p class="error">{error}</p>{/if}
  <label>Name<input type="text" bind:value={name} placeholder="Acme, Inc." /></label>
  <label>Website<input type="url" bind:value={website} placeholder="https://…" /></label>
  <label>Careers URL<input type="url" bind:value={careers_url} placeholder="https://…/careers" /></label>
  <div class="create-form__field"><span class="create-form__flabel">Domain(s)</span><DomainPicker bind:value={domain} /></div>
  <label>Business model(s)<input type="text" bind:value={business_model} placeholder="b2b" /></label>
  <div class="create-form__field"><span class="create-form__flabel">Size</span><Combobox placeholder="Size" bind:value={company_size} options={sizeOpts} /></div>
  <div class="create-form__field"><span class="create-form__flabel">Stage</span><Combobox placeholder="Stage" bind:value={stage} options={stageOpts} /></div>
  <div class="create-form__field"><span class="create-form__flabel">Remote</span><Combobox placeholder="Remote" bind:value={remote_policy} options={remoteOpts} /></div>
  <label>Location<input type="text" bind:value={location} placeholder="Remote, US" /></label>
  <label>Notes<textarea bind:value={notes} placeholder="Why listed…"></textarea></label>
  <div class="create-form__actions">
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={saving} onclick={submit}>{saving ? "Creating…" : "Create"}</button>
  </div>
</div>
