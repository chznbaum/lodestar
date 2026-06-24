# Contributing to Lodestar

A short guide to how we work. For project setup (toolchains, running the app, configuration), see
the [README](./README.md). For background on the architecture, see the
[Wiki](https://github.com/chznbaum/lodestar/wiki).

## Workflow at a glance

1. Branch off `main`.
2. Make your change; keep `main` green (all checks pass).
3. Open a pull request into `main`.
4. Squash and merge.

**No direct pushes to `main`** — every change lands through a PR.

## Branches

Branch off the latest `main` and name the branch `type/short-description`, where `type` matches the
commit type for the work:

```
feat/triage-surface
fix/dompurify-sanitize
docs/contributing
refactor/fit-scoring
chore/bump-deps
```

## Commits

We use [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): subject`.

- **Types:** `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`.
- **Scope** is the area touched, e.g. `pipeline`, `jobs`, `checks`, `fit`, `prompts`, `roles`,
  `security`, `docs`. Optional, but use it when it adds clarity.
- Keep the subject short and imperative ("add", not "added").

```
feat(checks): show per-step cache activity on the run-detail page
fix(security): sanitize rendered markdown with dompurify
```

## Pull requests

- Open the PR against `main`. **Review is encouraged but not blocking — you may merge your own PR.**
  Request a review when the change is non-trivial or you want a second pair of eyes.
- **Merge with "Squash and merge."** The PR collapses to a single commit on `main`, so give the PR
  title a Conventional Commit subject (`type(scope): …`) — that becomes the commit message.
- Keep PRs focused; smaller is easier to review and revert.

## Checks (must pass before merge)

Run these from the repo root before opening or merging a PR:

```sh
npm test                                                       # frontend unit tests (Vitest)
npm run check                                                  # Svelte + TypeScript type-check
cargo test  --manifest-path src-tauri/Cargo.toml               # backend tests (Rust)
cargo fmt   --manifest-path src-tauri/Cargo.toml --check       # Rust formatting
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings   # Rust lints
```

`cargo fmt` and `clippy` use default settings (no config files yet) — run `cargo fmt` without
`--check` to auto-fix formatting. CI (`.github/workflows/ci.yml`) runs all of these on every PR
into `main`, so keep them green locally to avoid a red build.

## Recording decisions

- **Architecture decisions** that are hard to reverse or likely to be questioned later belong in
  [`adr/`](./adr/) as a numbered record — not buried in a PR description.

## Where docs live

User- and architecture-facing documentation lives in the
[Wiki](https://github.com/chznbaum/lodestar/wiki). The README is the orientation; the wiki is the
reference. Update the relevant page when your change affects documented behavior.
