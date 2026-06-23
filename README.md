# Lodestar

A local-first job-search workbench (Tauri v2 + SvelteKit + Svelte 5 runes) over a plain-text
Obsidian vault. The backend (`src-tauri/`) owns the vault I/O and the job-fetch pipeline; the
frontend is the workspace UI.

## Product surfaces
- **Today** *(home)* — what needs you now: follow-ups due, upcoming interviews, debriefs to
  write, outreach to send, new roles to triage.
- **Triage** — move through newly-found roles one at a time; pursue / skip. Focused flow, not a
  grid.
- **Pipeline** — active applications as a board by stage; spot what's stalling.
- **Companies** — Action-queue home + company-workspace detail + view tabs + Checks inspector + Settings.
- **Network** — warm connections; who to reach out to; who's gone quiet.
- **Patterns** — what outcomes reveal: where rejections happen, which rounds go well, what gets
  skipped most.
- **Checks** - task runner log.
- **Settings** - API keys, and eventually other configurations.

## Docs

- [Model tiers](./docs/model-tiers.md) — how LLM stages map to capability tiers and how to swap models.
- [OpenRouter guardrails](./docs/openrouter-guardrails.md) — the account-side budget + prompt-injection guardrails the pipeline operationally depends on (set these up before a real run).

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
