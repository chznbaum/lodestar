# Lodestar

A local-first job-search workbench (Tauri v2 + SvelteKit + Svelte 5 runes) over a plain-text
Obsidian vault. The backend (`src-tauri/`) owns the vault I/O and the job-fetch pipeline; the
frontend is the workspace UI.

## Docs

- [Model tiers](./docs/model-tiers.md) — how LLM stages map to capability tiers and how to swap models.
- [OpenRouter guardrails](./docs/openrouter-guardrails.md) — the account-side budget + prompt-injection guardrails the pipeline operationally depends on (set these up before a real run).

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
