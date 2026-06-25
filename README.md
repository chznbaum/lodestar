# Lodestar

> *Job hunting is a numbers game. Lodestar lets you play it without phoning it in.*

[![CI](https://github.com/chznbaum/lodestar/actions/workflows/ci.yml/badge.svg)](https://github.com/chznbaum/lodestar/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-000000?logo=apple&logoColor=white)](#status)
[![Status: early development](https://img.shields.io/badge/status-early%20development-orange)](#status)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-2021-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)

A local-first desktop **job-search workbench**. Lodestar scrapes company career pages,
extracts structured role data with LLMs, and scores each role against your profile — so you can
run a high-volume application pipeline without losing the thread.

Everything lives in a plain-text [Obsidian](https://obsidian.md/) vault: companies, jobs,
profile, and run logs are Markdown files with YAML frontmatter. The vault is the source of
truth, so your data stays human-readable, editable in any tool, and version-control friendly.

> **Status:** early, active development. Built and tested on **macOS only** today (secret storage
> uses the Apple-native keychain). iOS is the longer-term intent, and the Tauri stack leaves the
> door open to other desktop platforms later.

📖 **Full documentation lives in the [GitHub Wiki](https://github.com/chznbaum/lodestar/wiki).**
This README is the orientation; the wiki is the reference.

## How it works

1. **Track companies.** Add a company (with its careers URL) to the vault.
2. **Discover roles.** A background pipeline scrapes the careers page, uses an LLM to structure
   the listings, prefilters out clearly-irrelevant roles, and writes new job stubs to the vault.
3. **Detail a role.** For a role you care about, the pipeline scrapes the full job description,
   structures it, detects missing facts (salary, tech stack, visa policy…) and optionally fills
   those gaps with LLM web research.
4. **Score the fit.** A scoring engine compares the role to your target criteria and experience,
   producing a 0–100 fit breakdown plus a written alignment narrative.
5. **Triage and apply.** You work the results through the UI.

The pipeline is a durable, retryable **task queue** (SQLite): each step is a discrete unit of work
that enqueues its successor, so a failure in a late stage never re-runs an expensive upstream
scrape. Every run is recorded as a **Check** with per-step telemetry and LLM cost.

See [Job-Fetch Pipeline](https://github.com/chznbaum/lodestar/wiki/Job%E2%80%90Fetch-Pipeline) and
[Fit Scoring Engine](https://github.com/chznbaum/lodestar/wiki/Fit-Scoring-Engine) for the details.

## Product surfaces

The UI is a navigation rail of "surfaces." Only some are built today:

| Surface | Status | Purpose |
| --- | --- | --- |
| **Companies** (home) | ✅ Built | Browse, filter, and search tracked companies and their roles; launch pipeline runs. |
| **Checks** | ✅ Built | Diagnostics log for pipeline runs — status, per-step activity, and LLM/scrape cost. |
| **Settings** | ✅ Built | API keys and vault configuration. |
| Today | ⏳ Planned | Daily dashboard: follow-ups, interviews, outreach due. |
| Triage | ⏳ Planned | Focused, one-role-at-a-time review of newly-found roles. |
| Pipeline | ⏳ Planned | Board of active applications by stage. |
| Network | ⏳ Planned | Warm connections and referrals. |
| Patterns | ⏳ Planned | Analytics on outcomes and where applications stall. |

More in [Product Surfaces & Navigation](https://github.com/chznbaum/lodestar/wiki/Product-Surface-&-Navigation).

## Tech stack

- **Shell:** [Tauri v2](https://v2.tauri.app/) (native Rust core + system WebView)
- **Frontend:** [SvelteKit](https://kit.svelte.dev/) + Svelte 5 runes, static SPA adapter, Vite
- **Backend:** Rust — vault I/O, the SQLite task queue (`rusqlite`), the file watcher (`notify`),
  and pipeline/LLM orchestration
- **Scraping:** [ScrapingBee](https://www.scrapingbee.com/) (with automatic proxy escalation)
- **LLM:** [OpenRouter](https://openrouter.ai/) (model per stage is configurable; defaults to Claude)
- **Secrets:** OS keychain via the `keyring` crate (keys never touch the vault or disk in plaintext)

## Getting started

### Prerequisites

- macOS
- [Rust](https://rustup.rs/) toolchain (2021 edition)
- [Node.js](https://nodejs.org/) (LTS) + npm
- API keys for **ScrapingBee** and **OpenRouter**

### Install & run

```sh
npm install            # frontend dependencies
npm run tauri dev      # starts Vite + compiles and launches the Tauri app
```

Cargo fetches the Rust dependencies on first build.

### First-run setup

1. **Pick a vault** — choose a folder (ideally an Obsidian vault) via the native picker. All
   entities are stored there as Markdown.
2. **Add API keys** — in **Settings**, paste your ScrapingBee and OpenRouter keys. They're written
   to the OS keychain (the UI can set them but never reads them back).
3. **Set guardrails first** — before running against real career pages, set a spend limit on your
   OpenRouter account. The pipeline feeds untrusted scraped text to LLMs, so review the
   prompt-injection handling in
   [LLM Integration & Prompt Engineering](https://github.com/chznbaum/lodestar/wiki/LLM-Integration-&-Prompt-Engineering).

### Build a release bundle

```sh
npm run tauri build
```

Full setup notes: [Getting Started & Configuration](https://github.com/chznbaum/lodestar/wiki/Getting-Started-&-Configuration).

## Configuration

Two storage locations, kept separate:

- **Vault directory** — your data (companies, jobs, profile, checks), as Markdown + YAML.
- **App config directory** — internal state. Holds the SQLite task queue and `config.json`, which
  maps each LLM pipeline stage to a capability **tier** (`Frontier` / `Balanced` / `Speed`). Tiers
  default to Claude models and are remappable to any OpenRouter slug.

## Project layout

```
src/                 SvelteKit frontend (routes/ surfaces, lib/ stores + logic)
src-tauri/src/       Rust backend
  pipeline/            task queue, step runner, run orchestration
  note.rs              Markdown + YAML round-trip vault I/O
  company.rs job.rs    entity structs (also domain/metro/competency/community)
  fit.rs               fit-scoring engine
  prompts.rs llm.rs    LLM prompts + OpenRouter client
  scraper.rs           ScrapingBee client
  sanitize.rs          HTML cleanup before LLM
  secrets.rs           OS keychain access
  watcher.rs           vault file watcher → live UI reload
  check.rs config.rs   run telemetry; pipeline/model config
adr/                 architecture decision records
agents.md            short orientation for AI coding agents
```

A fuller tour of the data model is in
[The Vault: Data Model & Persistence](https://github.com/chznbaum/lodestar/wiki/The-Vault:-Data-Model-&-Persistence),
and terms are defined in the [Glossary](https://github.com/chznbaum/lodestar/wiki/Glossary).

## Testing

```sh
npm test                                  # frontend (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml   # backend (Rust)
```

Rust pipeline tests run against `FakeScraper` / `FakeLlm` so they have no external side effects.
See [Testing Strategy](https://github.com/chznbaum/lodestar/wiki/Testing-Strategy).

## Contributing

Work happens on branches and lands through pull requests — see
[CONTRIBUTING.md](./CONTRIBUTING.md) for the branch/commit/PR conventions and the checks CI
enforces.
