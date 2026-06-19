//! Generic plain-text note primitives shared by every entity (company, job, check):
//! frontmatter/body splitting, field-level round-tripping writes, the single vault-write
//! choke point, and the filename → slug eligibility rule. No entity-specific knowledge lives here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Paths the app itself just wrote, each stamped with the time of the write. The vault
/// file-watcher (`watcher.rs`) consults this to skip the change events our OWN writes produce,
/// so an in-app edit or a pipeline write never echoes back as a spurious "external change".
/// Entries are short-lived: a genuine external edit arriving after the TTL is (correctly) NOT
/// suppressed.
static SELF_WRITES: LazyLock<Mutex<HashMap<PathBuf, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// How long a recorded self-write masks the matching watcher event — comfortably longer than the
/// watcher's debounce window, short enough that a later genuine edit to the same file isn't eaten.
const SELF_WRITE_TTL: Duration = Duration::from_secs(5);

/// The single vault-write choke point: write `contents` to `path`, recording it as a self-write
/// so the file-watcher ignores the resulting event. EVERY entity write (company/job/check) goes
/// through here — that invariant is what lets the watcher distinguish our writes from the user's.
pub fn write_note(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| e.to_string())?;
    if let Ok(mut m) = SELF_WRITES.lock() {
        m.insert(canonical(path), Instant::now());
    }
    Ok(())
}

/// Whether `path` was written by the app within `SELF_WRITE_TTL` (a self-write the watcher should
/// ignore). Consumes the matching record — so a real edit landing right after our write still
/// reloads — and evicts stale entries while it holds the lock.
pub fn was_self_write(path: &Path) -> bool {
    let key = canonical(path);
    let Ok(mut m) = SELF_WRITES.lock() else {
        return false;
    };
    let now = Instant::now();
    m.retain(|_, t| now.duration_since(*t) < SELF_WRITE_TTL);
    m.remove(&key).is_some()
}

/// Best-effort path normalization so a write and the watcher's event for the same file compare
/// equal despite symlinks / `.`-segments. Falls back to the path as-given when canonicalization
/// fails (e.g. the file was since removed — but the app only ever writes, never deletes, notes).
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Assumes the controlled vault note format: the first `\n---` after the opening
/// fence is the closing delimiter. Not a general YAML parser.
/// Split a note into (frontmatter_yaml, body). Returns ("", whole) if no frontmatter.
pub fn split_frontmatter(text: &str) -> (&str, &str) {
    let after = match text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) {
        Some(rest) => rest,
        None => return ("", text),
    };
    match after.split_once("\n---") {
        Some((fm, body)) => (fm, body.trim_start_matches(['\r', '\n']).trim_start()),
        None => ("", text),
    }
}

/// Replace `key: ...` inside the frontmatter, preserving everything else byte-for-byte.
/// Inserts the key just before the closing `---` if absent. Errors if there's no frontmatter.
pub fn set_frontmatter_field(text: &str, key: &str, value: &str) -> Result<String, String> {
    if key.contains(['\n', '\r']) || value.contains(['\n', '\r']) {
        return Err("frontmatter key/value must not contain newlines".into());
    }
    let body_start = text.find("\n---").ok_or("no frontmatter")?;
    let head = &text[..body_start]; // "---\n<fields...>" (no trailing newline)
    let rest = &text[body_start..]; // "\n---...body"
    let mut lines: Vec<String> = head.lines().map(String::from).collect();
    let prefix = format!("{key}:");
    let line = format!("{key}: {value}");
    match lines.iter().position(|l| l.trim_start().starts_with(&prefix)) {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }
    Ok(format!("{}{}", lines.join("\n"), rest))
}

/// Replace the note body (everything after the closing `---`), keeping the frontmatter.
/// Errors if the note has no frontmatter (refuses to clobber a malformed note).
pub fn set_body(text: &str, body: &str) -> Result<String, String> {
    let i = text.find("\n---").ok_or("no frontmatter")?;
    let after = &text[i + 1..]; // "---...body"
    let close_end = after.find('\n').map(|n| i + 1 + n + 1).unwrap_or(text.len());
    Ok(format!("{}\n{}\n", text[..close_end].trim_end(), body))
}

/// The slug for an eligible note filename, or None to skip it.
/// Skips non-`.md` files and `_`-prefixed files (templates / sidecars like `_jd/`, `_logos/`).
pub fn note_slug(file_name: &str) -> Option<String> {
    let stem = file_name.strip_suffix(".md")?;
    if stem.is_empty() || stem.starts_with('_') {
        return None;
    }
    Some(stem.to_string())
}

/// Convert a human name into the project's slug form: lowercase ASCII alphanumerics,
/// every other run collapsed to a single hyphen, no leading/trailing hyphen.
/// Non-ASCII characters are dropped (ASCII-safe slug rule). May return "" (caller must check).
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_hyphen = true;
        }
    }
    out
}

/// Read every eligible note under `dir`, parsing each via `parse(slug, text)`. Skips
/// non-`.md`/underscored files (per `note_slug`); a **missing dir yields an empty Vec**
/// (a present-but-unreadable dir errors). Parse errors are logged and that note skipped.
pub fn read_notes_in<T>(
    dir: &Path,
    parse: impl Fn(&str, &str) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {dir:?}: {e}")),
    };
    let mut out = Vec::new();
    for entry in rd {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let Some(slug) = note_slug(file_name) else {
            continue;
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match parse(&slug, &text) {
            Ok(v) => out.push(v),
            Err(e) => eprintln!("skip {slug}: {e}"),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_field_replaces_only_that_line() {
        let t = "---\nid: stripe\nstatus: active\nlast_checked:\n---\n\nbody\n";
        let out = set_frontmatter_field(t, "status", "paused").unwrap();
        assert!(out.contains("status: paused"));
        assert!(out.contains("id: stripe")); // untouched
        assert!(out.contains("\n\nbody\n")); // body untouched
        assert!(!out.contains("status: active"));
    }

    #[test]
    fn set_field_inserts_if_absent() {
        let t = "---\nid: stripe\nstatus: active\n---\n\nbody\n";
        let out = set_frontmatter_field(t, "last_checked", "2026-06-15").unwrap();
        assert!(out.contains("last_checked: 2026-06-15"));
        assert!(out.contains("status: active"));
    }

    #[test]
    fn set_body_replaces_body_keeps_frontmatter() {
        let t = "---\nid: stripe\nstatus: active\n---\n\nold body\n";
        let out = set_body(t, "new body").unwrap();
        assert!(out.starts_with("---\nid: stripe\nstatus: active\n---\n"));
        assert!(out.trim_end().ends_with("new body"));
        assert!(!out.contains("old body"));
    }

    #[test]
    fn set_field_does_not_match_key_with_shared_prefix() {
        let t = "---\nstatus: active\nstatus_detail: something\n---\n\nbody\n";
        let out = set_frontmatter_field(t, "status", "paused").unwrap();
        assert!(out.contains("status_detail: something")); // untouched
        assert!(out.contains("status: paused"));
        assert!(!out.contains("status: active"));
    }

    #[test]
    fn set_field_rejects_newlines() {
        let t = "---\nstatus: active\n---\n\nbody\n";
        assert!(set_frontmatter_field(t, "status", "active\ninjected: x").is_err());
        assert!(set_frontmatter_field(t, "ev\nil", "x").is_err());
    }

    #[test]
    fn set_body_errors_without_frontmatter() {
        assert!(set_body("just a body, no frontmatter\n", "new").is_err());
    }

    #[test]
    fn note_slug_skips_underscored_and_non_md() {
        assert_eq!(note_slug("stripe.md").as_deref(), Some("stripe"));
        assert_eq!(note_slug("_template.md"), None);
        assert_eq!(note_slug("notes.txt"), None);
        assert_eq!(note_slug(".md"), None);
    }

    #[test]
    fn read_notes_in_parses_skips_underscored_and_missing_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!("lodestar-readnotes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "---\nid: a\n---\n").unwrap();
        std::fs::write(dir.join("b.md"), "---\nid: b\n---\n").unwrap();
        std::fs::write(dir.join("_t.md"), "---\nid: t\n---\n").unwrap(); // skipped (underscored)
        std::fs::write(dir.join("readme.txt"), "x").unwrap(); // skipped (non-md)

        let mut slugs =
            read_notes_in(&dir, |slug, _text| Ok::<_, String>(slug.to_string())).unwrap();
        slugs.sort();
        assert_eq!(slugs, vec!["a".to_string(), "b".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
        // Missing dir -> empty Vec, never an error.
        let missing = std::path::Path::new("/no/such/lodestar/dir");
        assert!(read_notes_in(missing, |s, _| Ok::<_, String>(s.to_string()))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn slugify_matches_house_style() {
        assert_eq!(slugify("Stripe"), "stripe");
        assert_eq!(slugify("15Five"), "15five");
        assert_eq!(slugify("1Password"), "1password");
        assert_eq!(slugify("Solutions by Chazona"), "solutions-by-chazona");
        assert_eq!(slugify("  Acme, Inc.  "), "acme-inc");
        assert_eq!(slugify("!!!"), ""); // caller must reject empty
    }

    #[test]
    fn write_note_records_a_consumable_self_write() {
        let dir = std::env::temp_dir().join(format!("lodestar-selfwrite-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.md");

        write_note(&p, "---\nid: x\n---\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "---\nid: x\n---\n"); // it really wrote

        assert!(was_self_write(&p), "our own write is recognized as a self-write");
        assert!(!was_self_write(&p), "the record is consumed — a later edit is NOT suppressed");
        assert!(!was_self_write(&dir.join("never.md")), "a path we never wrote is never a self-write");

        std::fs::remove_dir_all(&dir).ok();
    }
}
