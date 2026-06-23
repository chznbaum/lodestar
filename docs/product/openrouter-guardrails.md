# OpenRouter guardrails

Lodestar's LLM work runs through [OpenRouter](https://openrouter.ai). Two of OpenRouter's account-side **[Guardrails](https://openrouter.ai/blog/announcements/guardrails/)** are *operational dependencies* of the app — set them once in your OpenRouter workspace and the pipeline leans on them. Guardrails are configured under **Workspaces → Guardrails** in the OpenRouter dashboard (or via its Management API) at the workspace, team, or API-key level; they apply with **no code changes** here.

## 1. Budget enforcement

- Set a spending cap with a **daily / weekly / monthly** reset — via a **workspace Guardrails budget** (per member and/or key; the lower limit wins) and/or a **per-key credit limit** minted through OpenRouter's Provisioning keys.
- Exceeding it returns an error whose body is `{ error: { code, message, metadata? } }`. Exhausted credits / a negative balance → **HTTP `402`** (even for `:free` models). A *per-key* budget cap has also been observed to surface as **`403`** ("API key budget limit exceeded …") — the same status the guardrails use — so **treat both `402` and `403` as "stop," and read `error.message` to tell a budget hit from a guardrail block.** (The `403`-budget case is third-party-reported, not canonical docs; handle it defensively.)
- `OpenRouterLlm::complete` already fails on any non-2xx, returning `openrouter returned <status>: <body>` — so the runner records the full message on the `checks/` note (visible on the **Checks** surface), retries with backoff, then marks the run **`failed`**. A hit budget is a **clean stop**, not a silent overspend.

## 2. Prompt-injection guardrail

Careers-page HTML is **untrusted input**:

| Layer | Where | What it does |
|-------|-------|--------------|
| OpenRouter **Prompt Injection** guardrail | OpenRouter account | >30 regex patterns derived from the OWASP *LLM Prompt Injection Prevention* cheat sheet; catches evasion (typoglycemia, encoding tricks). Modes: **Flag** (record only), **Redact** (`[PROMPT_INJECTION]`), **Block** (reject with **HTTP `403`** before the model runs). |
| `sanitize.rs` | this app | Strips scripts/styles/hidden + zero-width nodes and **fences** the visible text as data. The sole structural gate between scraped bytes and the LLM. |
| Prompt framing | `prompts.rs` | The system prompt frames the fenced block as **DATA, never instructions**. |

**Enable the Prompt Injection guardrail** on the workspace/key the app uses. OpenRouter calls it **non-exhaustive**, so start in **Flag** mode and move to **Block** once false positives are ruled out. It is the cheap pre-model layer the design assumes; `sanitize.rs` is the in-app complement, not a replacement. A blocked request returns **`403`** — disambiguate from a budget `403` via `error.message` — surfaced and recorded like any step error.

## How the app reads cost

OpenRouter **always** returns a `usage` object with per-call cost — no request parameter needed (`usage: { include: true }` and `stream_options: { include_usage: true }` are deprecated no-ops). The runner reads that cost into `Step.cost`; the **Checks** surface rolls those into the credits/spend readout (ScrapingBee in **credits**, OpenRouter in **dollars**).

See also: [Model tiers](./model-tiers.md) · OpenRouter docs for [limits & 402](https://openrouter.ai/docs/api/reference/limits) and [usage accounting](https://openrouter.ai/docs/cookbook/administration/usage-accounting).
