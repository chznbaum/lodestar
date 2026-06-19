<script lang="ts">
  import { SECRET_KEYS, setSecret, secretPresent } from "$lib/secrets";

  let keyValues = $state<Record<string, string>>(
    Object.fromEntries(SECRET_KEYS.map((k) => [k.key, ""])),
  );
  let keyPresent = $state<Record<string, boolean>>({});
  let savingKey = $state<string | null>(null);

  // Read each key's presence once on mount. The empty dependency list (no reactive
  // values referenced inside) means this effect runs exactly once and never re-fires.
  $effect(() => {
    for (const { key } of SECRET_KEYS) {
      secretPresent(key).then((p) => (keyPresent[key] = p));
    }
  });

  async function saveKey(key: string) {
    savingKey = key;
    try {
      await setSecret(key, keyValues[key]);
      keyPresent[key] = true;
      keyValues[key] = ""; // clear the input; the value is never read back
    } catch (e) {
      console.error("set_secret failed", e);
    } finally {
      savingKey = null;
    }
  }
</script>

<section class="settings">
  <h1>Settings</h1>

  <section class="settings__keys">
    <div class="panel__head">API keys <span class="sub">stored in your OS keychain — never shown back or written to the vault</span></div>
    {#each SECRET_KEYS as { key, label } (key)}
      <div class="keyfield">
        <label for="key-{key}">{label}</label>
        <input
          id="key-{key}"
          type="password"
          autocomplete="off"
          bind:value={keyValues[key]}
          placeholder={keyPresent[key] ? "•••••••• (set — enter to replace)" : "not set"}
        />
        <button class="btn sm" disabled={!keyValues[key] || savingKey === key} onclick={() => saveKey(key)}>
          {savingKey === key ? "Saving…" : "Save"}
        </button>
        <span class="keyfield__status">{keyPresent[key] ? "✓ set" : "— not set"}</span>
      </div>
    {/each}
  </section>
</section>
