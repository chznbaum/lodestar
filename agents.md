## Purpose
Automated job search assistant that scrapes career pages, extracts structured data via LLMs, and calculates candidate-job alignment. Designed for individual professionals managing a high-volume application pipeline.

## Key Technologies
Rust, Tauri v2, Svelte, SQLite, ScrapingBee, OpenRouter (LLM), Serde, Rusqlite, Keychain API.

## Top-Level Structure
* `src-tauri/src/pipeline/`: Core execution logic and task orchestration.
* `src-tauri/src/prompts.rs`: LLM prompt engineering and schema validation.
* `src-tauri/src/job.rs`: Job entity state machine and Markdown persistence.
* `src-tauri/src/fit.rs`: Scoring engine for candidate-job compatibility.
* `src-tauri/src/secrets.rs`: Secure OS-level API key management.
* `src-tauri/src/profile.rs`: User criteria and accomplishment parsing.
* `src-tauri/src/check.rs`: Durable run telemetry and execution history.
* `src/routes/`: Svelte-based frontend views and triage UI.
* `src/lib/pipeline.ts`: Frontend-to-backend pipeline bridge.

## Key Concepts
* **Vault**: Local directory of Markdown files for data persistence.
* **Check**: A durable record of a single pipeline execution run.
* **Step**: Atomic unit of work (scrape, LLM, script) within a Check.
* **Job Stub**: Minimal job record created during initial discovery.
* **Fit Breakdown**: Granular 0–100 scores across five suitability dimensions.
* **Dealbreaker**: Hard constraint (e.g., visa) that collapses fit score to zero.
* **Gaps**: Missing data points in a JD requiring LLM web research.
* **Recall-oriented**: Permissive filtering strategy to avoid false negatives.
* **Monotonic Status**: State machine preventing pipeline downgrades.
* **Disposition**: Resolved action for failed tasks (Retry, Terminal, Reenqueue).
* **Stealth Proxy**: High-tier scraping retry for anti-bot bypass.
* **Step Strip**: UI visualization of real-time pipeline progress.
* **YOE Reducer**: Score penalty applied when experience is below minimums.
* **Data Fence**: Security markers isolating untrusted scraped text in prompts.
