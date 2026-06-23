# Model tiers

Lodestar's LLM work runs through [OpenRouter](https://openrouter.ai), and every stage of a pipeline is assigned a **capability tier** rather than a hard-coded model. This keeps model choice in one place and lets you swap models (including non-Anthropic ones) without touching pipeline code.

There are two layers:

1. **Stage → tier** — *intrinsic* to what a stage does (objective extraction vs. nuanced reasoning). This lives in code (`src-tauri/src/config.rs`, `tier_for_stage`) and is the source of truth this page mirrors. It is deliberately **not** a per-stage setting: picking a model per stage is exactly the fiddling this design avoids.
2. **Tier → model** — the single knob you edit. It lives in `config.json` in the app config dir (never in your vault), and is what a future settings screen will expose.

## Tiers and their default models

| Tier | Default model | Rough cost (per 1M tokens, in/out) | Used for |
|------|---------------|------------------------------------|----------|
| `frontier` | `anthropic/claude-opus-4.8` | $5 / $25 | Reasoning, judgment, anything nuanced |
| `balanced` | `anthropic/claude-sonnet-4.6` | $3 / $15 | Reliable, high-volume objective extraction |
| `speed` | `anthropic/claude-haiku-4.5` | $1 / $5 | Reserved — no pipeline stage uses it yet |

Defaults are Anthropic-prioritized but any OpenRouter model slug is valid.

## Which stage runs on which tier (job-fetch pipeline)

| Stage | Tier | Why |
|-------|------|-----|
| `structure-listings` | `balanced` | High-volume, objective "what roles are listed here?" extraction |
| `structure-jd` | `frontier` | Extraction, but over nuanced text (e.g. Java vs JavaScript) that feeds alignment |
| `research-gaps` | `frontier` | Targeted research whose output the alignment step reasons over |
| `alignment` | `frontier` | The crown jewel: an honest fit score + positioning over your real accomplishments |
| *any other / future stage* | `frontier` | Quality-first default — a stage is only downgraded once it's proven objective-extraction |

**Cost expectation:** in a typical run, the high-volume step (`structure-listings`, one call per listing across every company) is the cheaper `balanced` tier; the lower-volume-but-higher-stakes steps (per *selected* role) run on `frontier`. The `frontier` default for anything unclassified means new work is high-quality by default and you opt *into* cheaper tiers deliberately.

## Changing models

Edit the `tiers` map in `config.json`:

```jsonc
{
  "tiers": {
    "frontier": "anthropic/claude-opus-4.8",
    "balanced": "anthropic/claude-sonnet-4.6",
    "speed":    "anthropic/claude-haiku-4.5"
  },
  "schedule_enabled": false
}
```

Any change applies to **every** stage on that tier. To change *which* tier a stage uses, edit `tier_for_stage` in `src-tauri/src/config.rs` (and update the table above to match).

## Prompt caching (alignment step)

The `alignment` step re-sends your entire candidate profile — positioning, targets, career history, accomplishments, community — on every job and every re-score. That profile is byte-identical across roles, so it leads the request as a **cached reference prefix** (the job-specific content — fit breakdown, company, the job description, research notes — stays in the uncached suffix). The prefix carries a single `cache_control` breakpoint with a **1-hour TTL**, so the model reads it from cache instead of re-processing it on the next call within the hour.

What this means in practice:

- **Profile edits take effect on the next score.** The cache is keyed to the *exact bytes* of the prefix, so a changed profile can't match a stale cache entry — the next score after an edit pays a fresh cache write, then reads from cache for an hour. No stale-profile reads are possible, and there's no invalidation step to run.
- **Unchanged-profile re-scores within the hour read from cache.** Re-scoring several roles back-to-back (or re-running the same one) reuses the cached profile, so only the per-role suffix is freshly processed.
- **Cost is already correct.** OpenRouter's reported `usage.cost` already nets the cache discount, so spend telemetry needs no adjustment. The captured `cache_read_tokens` / `cache_write_tokens` are visibility only (they prove caching is engaging: a read `> 0` means the prefix was served from cache).
