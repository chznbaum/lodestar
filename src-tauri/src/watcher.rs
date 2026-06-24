//! Vault file-watcher: emits `record:changed { kind, slug }` for EXTERNAL edits to the four
//! entity dirs, so the UI live-reloads notes changed outside the app (e.g. in Obsidian).
//!
//! The app's OWN writes are skipped via `note::was_self_write` — every vault write goes through
//! the single `note::write_note` choke point, which records the path so the resulting change
//! event is recognized as ours and dropped (no reload echo). Pipeline writes (checks/jobs during
//! a run) are self-writes too; the UI learns about those from the `run:*` events instead.
//!
//! The pure path→(kind, slug) classification is unit-tested here; the `notify` wiring is thin I/O.

use crate::note::{note_slug, was_self_write};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// The entity dirs we watch, each mapped to the `kind` reported to the frontend. `profile/` and
/// the app-data queue DB are deliberately absent — only user-facing entity notes live-reload.
const WATCHED: &[(&str, &str)] = &[
    ("companies", "company"),
    ("jobs", "job"),
    ("domains", "domain"),
    ("checks", "check"),
];

/// Coalesce events into the debounce window before emitting; comfortably under the self-write TTL.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// Payload for the `record:changed` event consumed by the frontend's vault-sync layer.
#[derive(Clone, Serialize)]
struct RecordChanged {
    kind: String,
    slug: String,
}

/// The live watcher, kept alive in Tauri-managed state. The `Debouncer` guard owns its OS watcher
/// + debounce thread; dropping it (e.g. when re-pointed at a new vault) stops watching.
type VaultDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;
pub struct WatcherState(pub Mutex<Option<VaultDebouncer>>);

impl WatcherState {
    pub fn new() -> Self {
        WatcherState(Mutex::new(None))
    }
}

/// Classify a changed path into `(kind, slug)` if it is an eligible entity note, else `None`.
/// Eligible = a `.md` file (not underscored — `note_slug` enforces the template/sidecar rule)
/// sitting directly in one of the watched entity dirs. Everything else (other dirs, nested
/// sidecars like `companies/_jd/…`, non-md files) is ignored.
pub fn classify_change(changed: &Path) -> Option<(&'static str, String)> {
    let slug = note_slug(changed.file_name()?.to_str()?)?;
    let parent = changed.parent()?.file_name()?.to_str()?;
    let kind = WATCHED
        .iter()
        .find(|(dir, _)| *dir == parent)
        .map(|(_, k)| *k)?;
    Some((kind, slug))
}

/// Start (or re-point) the vault file-watcher for `vault_path`. Invoked from the frontend once a
/// vault path is known — the path lives in the frontend, so the setup hook can't start this. The
/// `Debouncer` is parked in managed state; calling again replaces it (stopping the prior watch).
#[tauri::command]
pub fn start_vault_watcher(
    app: AppHandle,
    state: State<'_, WatcherState>,
    vault_path: String,
) -> Result<(), String> {
    let base = std::path::PathBuf::from(&vault_path);
    let emitter = app.clone();

    let mut debouncer = new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
        let Ok(events) = res else {
            return; // watcher-level errors: nothing actionable to emit
        };
        // One save can surface as several events; collapse to one emit per (kind, slug) per batch.
        let mut seen: HashSet<(&'static str, String)> = HashSet::new();
        for ev in events {
            if matches!(ev.kind, EventKind::Access(_)) {
                continue; // reads aren't changes
            }
            for path in &ev.paths {
                let Some((kind, slug)) = classify_change(path) else {
                    continue;
                };
                if was_self_write(path) {
                    continue; // our own write — the UI already knows (optimistic edit / run event)
                }
                if !seen.insert((kind, slug.clone())) {
                    continue;
                }
                let _ = emitter.emit(
                    "record:changed",
                    RecordChanged {
                        kind: kind.to_string(),
                        slug,
                    },
                );
            }
        }
    })
    .map_err(|e| e.to_string())?;

    // Watch each entity dir non-recursively (direct note files only; nested sidecars are ignored).
    // Ensure the dirs exist first — the app creates checks/jobs lazily, but watching must not race
    // their first run, and an unwatched-because-absent dir would silently never live-reload.
    for (dir, _) in WATCHED {
        let p = base.join(dir);
        std::fs::create_dir_all(&p).map_err(|e| format!("ensure {p:?}: {e}"))?;
        debouncer
            .watch(&p, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;
    }

    *state.0.lock().map_err(|e| e.to_string())? = Some(debouncer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_each_entity_note_by_its_dir() {
        assert_eq!(
            classify_change(Path::new("/v/companies/stripe.md")),
            Some(("company", "stripe".to_string()))
        );
        assert_eq!(
            classify_change(Path::new("/v/jobs/senior-engineer-acme.md")),
            Some(("job", "senior-engineer-acme".to_string()))
        );
        assert_eq!(
            classify_change(Path::new("/v/domains/fintech.md")),
            Some(("domain", "fintech".to_string()))
        );
        assert_eq!(
            classify_change(Path::new("/v/checks/2026-06-18-0001.md")),
            Some(("check", "2026-06-18-0001".to_string()))
        );
    }

    #[test]
    fn ignores_templates_non_md_and_unwatched_dirs() {
        assert!(classify_change(Path::new("/v/companies/_template.md")).is_none()); // underscored
        assert!(classify_change(Path::new("/v/jobs/notes.txt")).is_none()); // non-md
        assert!(classify_change(Path::new("/v/profile/target_criteria.md")).is_none()); // unwatched dir
        assert!(classify_change(Path::new("/v/companies/_jd/jd.md")).is_none()); // nested sidecar dir
        assert!(classify_change(Path::new("/v/checks")).is_none()); // a dir itself, no .md
    }
}
