import { invoke } from "@tauri-apps/api/core";

// The whitelisted API keys (mirrors SECRET_KEYS in src-tauri/src/secrets.rs). The app sets
// them into the OS keychain and reports presence; it never reads a key's value back.
export const SECRET_KEYS = [
  { key: "scrapingbee_api_key", label: "ScrapingBee API key" },
  { key: "openrouter_api_key", label: "OpenRouter API key" },
] as const;

/** Store a key in the OS keychain (via the app's keyring, so the format round-trips). */
export function setSecret(key: string, value: string): Promise<void> {
  return invoke<void>("set_secret", { key, value });
}

/** Whether a key is set — never returns the value itself. */
export function secretPresent(key: string): Promise<boolean> {
  return invoke<boolean>("secret_present", { key });
}
