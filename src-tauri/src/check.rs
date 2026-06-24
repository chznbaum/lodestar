//! The `checks/` run→steps job-queue record (redesign §2.3). App-owned notes, so a
//! whole-note serde round-trip (not field-level edits). Mirrors `company.rs` style.
// Public write helpers (`render_check_note`, `write_check`, `append_step`) are called by the
// future pipeline (Phase J); suppress the dead-code lint until those callers exist.
#![allow(dead_code)]

use crate::note::{self, split_frontmatter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub stage: String,
    pub class: String,
    pub target: String,
    pub status: String,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<i64>,
    /// Cache **read** tokens for an llm step — from `LlmResponse.cache_read_tokens` (OpenRouter's
    /// `usage.prompt_tokens_details.cached_tokens`). `> 0` proves the cached prefix was served from
    /// cache. Observability only (`cost` already nets the discount). `None` for non-llm steps and
    /// for llm steps that don't cache; not emitted when absent (mirrors `cost`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<i64>,
    /// Cache **write** tokens for an llm step — from `LlmResponse.cache_write_tokens`. `> 0` on the
    /// first call / each 1h re-warm. `None` when there was no cache activity; not emitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
    /// Recorded issues when status is `"warning"`: core work succeeded but with noted problems.
    /// Empty on ok/failed steps; not emitted when empty (skip_serializing_if).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Check {
    pub slug: String,
    pub kind: String,
    pub trigger: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration: Option<String>,
    /// The single entity this run is about — a company slug for `job_check`, a job slug for
    /// `job_detail`. `kind` is the type discriminator; one run, one subject (never a list).
    pub subject: String,
    pub roles_found: u32,
    pub errors: u32,
    pub steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Front {
    kind: Option<String>,
    trigger: Option<String>,
    status: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    duration: Option<String>,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    roles_found: u32,
    #[serde(default)]
    errors: u32,
    #[serde(default)]
    steps: Vec<Step>,
}

pub fn parse_check(slug: &str, text: &str) -> Result<Check, String> {
    let (fm, _body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    Ok(Check {
        slug: slug.to_string(),
        kind: f.kind.unwrap_or_else(|| "job_check".into()),
        trigger: f.trigger.unwrap_or_else(|| "manual".into()),
        status: f.status.unwrap_or_else(|| "running".into()),
        started_at: f.started_at,
        finished_at: f.finished_at,
        duration: f.duration,
        subject: f.subject,
        roles_found: f.roles_found,
        errors: f.errors,
        steps: f.steps,
    })
}

/// Build a complete checks note: run fields + steps as one frontmatter block, then a
/// regenerated `## Summary` body. `id` is emitted (mirrors the company `id:` convention).
pub fn render_check_note(check: &Check) -> String {
    #[derive(Serialize)]
    struct Fm<'a> {
        id: &'a str,
        kind: &'a str,
        trigger: &'a str,
        status: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        started_at: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        finished_at: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<&'a str>,
        subject: &'a str,
        roles_found: u32,
        errors: u32,
        steps: &'a [Step],
    }
    let fm = Fm {
        id: &check.slug,
        kind: &check.kind,
        trigger: &check.trigger,
        status: &check.status,
        started_at: check.started_at.as_deref(),
        finished_at: check.finished_at.as_deref(),
        duration: check.duration.as_deref(),
        subject: &check.subject,
        roles_found: check.roles_found,
        errors: check.errors,
        steps: &check.steps,
    };
    let yaml = serde_yaml::to_string(&fm).expect("check frontmatter serializes");
    let summary = format!(
        "{} {} · {} roles found · {} errors",
        check.kind, check.subject, check.roles_found, check.errors,
    );
    format!("---\n{yaml}---\n\n## Summary\n\n{summary}\n")
}

/// `<vault>/checks/<id>.md`. Rejects ids that aren't a plain slug (no `/`, `\`, leading `.`).
fn check_path(vault_path: &str, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.contains(['/', '\\']) || id.starts_with('.') {
        return Err(format!("invalid check id {id:?}"));
    }
    Ok(Path::new(vault_path)
        .join("checks")
        .join(format!("{id}.md")))
}

pub fn write_check(vault_path: &str, check: &Check) -> Result<(), String> {
    let p = check_path(vault_path, &check.slug)?;
    note::write_note(&p, &render_check_note(check))
}

/// Append one step to an existing run and re-persist it (the queue projects each step here).
pub fn append_step(vault_path: &str, run_id: &str, step: Step) -> Result<Check, String> {
    let p = check_path(vault_path, run_id)?;
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
    let mut check = parse_check(run_id, &text)?;
    check.steps.push(step);
    write_check(vault_path, &check)?;
    Ok(check)
}

#[tauri::command]
pub fn get_check(vault_path: String, id: String) -> Result<Check, String> {
    let p = check_path(&vault_path, &id)?;
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
    parse_check(&id, &text)
}

#[derive(Debug, Serialize)]
pub struct CheckSummary {
    pub slug: String,
    pub kind: String,
    pub trigger: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration: Option<String>,
    pub subject: String,
    pub roles_found: u32,
    pub step_count: usize,
    pub failed_count: usize,
    /// Cost tally, unit implied by step `class`: ScrapingBee credits + OpenRouter micro-dollars.
    pub credits: u32,
    pub usd_micro: i64,
}

impl CheckSummary {
    fn from(c: &Check) -> Self {
        CheckSummary {
            slug: c.slug.clone(),
            kind: c.kind.clone(),
            trigger: c.trigger.clone(),
            status: c.status.clone(),
            started_at: c.started_at.clone(),
            finished_at: c.finished_at.clone(),
            duration: c.duration.clone(),
            subject: c.subject.clone(),
            roles_found: c.roles_found,
            step_count: c.steps.len(),
            failed_count: c.steps.iter().filter(|s| s.status == "failed").count(),
            credits: c
                .steps
                .iter()
                .filter(|s| s.class == "scrape")
                .filter_map(|s| s.cost)
                .sum::<i64>() as u32,
            usd_micro: c
                .steps
                .iter()
                .filter(|s| s.class == "llm" || s.class == "llm+web")
                .filter_map(|s| s.cost)
                .sum::<i64>(),
        }
    }
}

#[tauri::command]
pub fn list_checks(vault_path: String) -> Result<Vec<CheckSummary>, String> {
    let dir = Path::new(&vault_path).join("checks");
    let mut runs = note::read_notes_in(&dir, parse_check)?;
    // Newest first; fall back to id (date-prefixed) when started_at is absent.
    runs.sort_by(|a, b| {
        let ka = a.started_at.as_deref().unwrap_or(&a.slug);
        let kb = b.started_at.as_deref().unwrap_or(&b.slug);
        kb.cmp(ka)
    });
    Ok(runs.iter().map(CheckSummary::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "---\nid: 2026-06-17-0001\nkind: job_check\ntrigger: manual\nstatus: complete\nstarted_at: 2026-06-17T10:00:00\nsubject: stripe\nroles_found: 2\nerrors: 0\nsteps:\n  - stage: careers-scrape\n    class: scrape\n    target: stripe\n    status: ok\n    attempts: 1\n    cost: 5\n  - stage: structure-listings\n    class: llm\n    target: stripe\n    status: ok\n    attempts: 1\n---\n\n## Summary\n\nstripe: 2 roles\n";

    #[test]
    fn parses_run_with_steps() {
        let c = parse_check("2026-06-17-0001", RUN).unwrap();
        assert_eq!(c.slug, "2026-06-17-0001");
        assert_eq!(c.kind, "job_check");
        assert_eq!(c.status, "complete");
        assert_eq!(c.subject, "stripe");
        assert_eq!(c.roles_found, 2);
        assert_eq!(c.steps.len(), 2);
        assert_eq!(c.steps[0].stage, "careers-scrape");
        assert_eq!(c.steps[0].cost, Some(5));
        assert_eq!(c.steps[1].class, "llm");
    }

    #[test]
    fn parse_defaults_empty_steps_and_counts() {
        let t = "---\nid: r2\nkind: job_check\ntrigger: scheduled\nstatus: running\n---\n";
        let c = parse_check("r2", t).unwrap();
        assert!(c.steps.is_empty());
        assert_eq!(c.roles_found, 0);
        assert!(c.subject.is_empty());
    }

    #[test]
    fn render_round_trips_through_parse() {
        let c = parse_check("2026-06-17-0001", RUN).unwrap();
        let text = render_check_note(&c);
        assert!(text.starts_with("---\n"));
        assert!(text.contains("## Summary"));
        let again = parse_check("2026-06-17-0001", &text).unwrap();
        assert_eq!(again.kind, "job_check");
        assert_eq!(again.status, "complete");
        assert_eq!(again.steps.len(), 2);
        assert_eq!(again.steps[0].stage, "careers-scrape");
        assert_eq!(again.steps[0].cost, Some(5));
        assert_eq!(again.roles_found, 2);
    }

    fn empty_run(slug: &str) -> Check {
        Check {
            slug: slug.into(),
            kind: "job_check".into(),
            trigger: "manual".into(),
            status: "running".into(),
            started_at: Some("2026-06-17T10:00:00".into()),
            finished_at: None,
            duration: None,
            subject: "stripe".into(),
            roles_found: 0,
            errors: 0,
            steps: vec![],
        }
    }

    #[test]
    fn write_then_append_step_persists_and_reparses() {
        let dir = std::env::temp_dir().join(format!("lodestar-chk-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();

        write_check(&vault, &empty_run("2026-06-17-0001")).unwrap();
        let step = Step {
            stage: "careers-scrape".into(),
            class: "scrape".into(),
            target: "stripe".into(),
            status: "ok".into(),
            attempts: 1,
            started_at: None,
            finished_at: None,
            error: None,
            cost: Some(5),
            cache_read_tokens: None,
            cache_write_tokens: None,
            warnings: vec![],
        };
        let updated = append_step(&vault, "2026-06-17-0001", step).unwrap();
        assert_eq!(updated.steps.len(), 1);

        let reread = get_check(vault, "2026-06-17-0001".into()).unwrap();
        assert_eq!(reread.steps.len(), 1);
        assert_eq!(reread.steps[0].stage, "careers-scrape");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn summary_tallies_credits_and_usd_by_class() {
        let mut c = empty_run("2026-06-18-0001");
        c.steps = vec![
            Step {
                stage: "careers-scrape".into(),
                class: "scrape".into(),
                target: "x".into(),
                status: "ok".into(),
                attempts: 1,
                started_at: None,
                finished_at: None,
                error: None,
                cost: Some(25),
                cache_read_tokens: None,
                cache_write_tokens: None,
                warnings: vec![],
            },
            Step {
                stage: "structure-listings".into(),
                class: "llm".into(),
                target: "x".into(),
                status: "ok".into(),
                attempts: 1,
                started_at: None,
                finished_at: None,
                error: None,
                cost: Some(500_000),
                cache_read_tokens: None,
                cache_write_tokens: None,
                warnings: vec![],
            }, // $0.50 in micro-dollars
            Step {
                stage: "pre-filter".into(),
                class: "script".into(),
                target: "x".into(),
                status: "ok".into(),
                attempts: 1,
                started_at: None,
                finished_at: None,
                error: None,
                cost: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                warnings: vec![],
            },
        ];
        let s = CheckSummary::from(&c);
        assert_eq!(s.credits, 25); // scrape steps only
        assert_eq!(s.usd_micro, 500_000); // llm steps only ($0.50)
    }

    #[test]
    fn list_checks_summarizes_sorts_desc_and_skips_underscored() {
        let dir = std::env::temp_dir().join(format!("lodestar-chklist-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();

        let mut older = empty_run("2026-06-16-0001");
        older.started_at = Some("2026-06-16T09:00:00".into());
        let mut newer = empty_run("2026-06-17-0001");
        newer.started_at = Some("2026-06-17T09:00:00".into());
        newer.roles_found = 3;
        newer.steps = vec![
            Step {
                stage: "careers-scrape".into(),
                class: "scrape".into(),
                target: "stripe".into(),
                status: "ok".into(),
                attempts: 1,
                started_at: None,
                finished_at: None,
                error: None,
                cost: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                warnings: vec![],
            },
            Step {
                stage: "jd-scrape".into(),
                class: "scrape".into(),
                target: "x".into(),
                status: "failed".into(),
                attempts: 2,
                started_at: None,
                finished_at: None,
                error: Some("timeout".into()),
                cost: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                warnings: vec![],
            },
        ];
        write_check(&vault, &older).unwrap();
        write_check(&vault, &newer).unwrap();
        std::fs::write(
            dir.join("checks").join("_draft.md"),
            render_check_note(&older),
        )
        .unwrap(); // skipped

        let list = list_checks(vault).unwrap();
        let ids: Vec<_> = list.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(ids, vec!["2026-06-17-0001", "2026-06-16-0001"]); // newest first
        assert_eq!(list[0].step_count, 2);
        assert_eq!(list[0].failed_count, 1);
        assert_eq!(list[0].subject, "stripe");
        assert_eq!(list[0].roles_found, 3);

        std::fs::remove_dir_all(&dir).ok();
        assert!(list_checks("/no/such/vault".into()).unwrap().is_empty());
    }

    // ── Warning-step tests (TDD RED → GREEN) ────────────────────────────────

    #[test]
    fn step_with_warning_status_and_warnings_round_trips() {
        // Build a check that has a warning step, render it, parse it back:
        // status and warnings must survive the round-trip intact.
        let mut c = empty_run("2026-06-21-warn");
        c.steps = vec![Step {
            stage: "research-gaps".into(),
            class: "llm".into(),
            target: "stripe".into(),
            status: "warning".into(),
            attempts: 1,
            started_at: None,
            finished_at: None,
            error: None,
            cost: Some(1_000),
            cache_read_tokens: None,
            cache_write_tokens: None,
            warnings: vec![
                "rejected countries: expected array, got string".into(),
                "missing equity field".into(),
            ],
        }];
        let rendered = render_check_note(&c);
        let parsed = parse_check("2026-06-21-warn", &rendered).unwrap();
        assert_eq!(parsed.steps.len(), 1);
        assert_eq!(parsed.steps[0].status, "warning");
        assert_eq!(
            parsed.steps[0].warnings,
            vec![
                "rejected countries: expected array, got string".to_string(),
                "missing equity field".to_string(),
            ]
        );
        assert!(parsed.steps[0].error.is_none());
    }

    #[test]
    fn ok_step_emits_no_warnings_key() {
        // An ok step must NOT emit a `warnings:` key in the serialized YAML.
        let mut c = empty_run("2026-06-21-no-warn");
        c.steps = vec![Step {
            stage: "careers-scrape".into(),
            class: "scrape".into(),
            target: "stripe".into(),
            status: "ok".into(),
            attempts: 1,
            started_at: None,
            finished_at: None,
            error: None,
            cost: Some(5),
            cache_read_tokens: None,
            cache_write_tokens: None,
            warnings: vec![],
        }];
        let rendered = render_check_note(&c);
        assert!(
            !rendered.contains("warnings:"),
            "ok step must not emit warnings key; got:\n{rendered}"
        );
    }

    // ── Cache-token tests (Task 2a, TDD RED → GREEN) ─────────────────────────

    #[test]
    fn step_with_cache_tokens_round_trips() {
        // An alignment step carrying cache read/write counts must survive
        // render_check_note → parse_check intact.
        let mut c = empty_run("2026-06-23-cache");
        c.steps = vec![Step {
            stage: "alignment".into(),
            class: "llm".into(),
            target: "stripe".into(),
            status: "ok".into(),
            attempts: 1,
            started_at: None,
            finished_at: None,
            error: None,
            cost: Some(4_200),
            cache_read_tokens: Some(6_600),
            cache_write_tokens: Some(7_000),
            warnings: vec![],
        }];
        let rendered = render_check_note(&c);
        let parsed = parse_check("2026-06-23-cache", &rendered).unwrap();
        assert_eq!(parsed.steps.len(), 1);
        assert_eq!(parsed.steps[0].cache_read_tokens, Some(6_600));
        assert_eq!(parsed.steps[0].cache_write_tokens, Some(7_000));
    }

    #[test]
    fn step_without_cache_tokens_emits_no_keys() {
        // A step with no cache activity must NOT emit cache_read_tokens/cache_write_tokens
        // keys (skip_serializing_if = "Option::is_none", mirroring `cost`).
        let mut c = empty_run("2026-06-23-no-cache");
        c.steps = vec![Step {
            stage: "careers-scrape".into(),
            class: "scrape".into(),
            target: "stripe".into(),
            status: "ok".into(),
            attempts: 1,
            started_at: None,
            finished_at: None,
            error: None,
            cost: Some(5),
            cache_read_tokens: None,
            cache_write_tokens: None,
            warnings: vec![],
        }];
        let rendered = render_check_note(&c);
        assert!(
            !rendered.contains("cache_read_tokens:") && !rendered.contains("cache_write_tokens:"),
            "None cache fields must not emit keys; got:\n{rendered}"
        );
    }

    #[test]
    fn old_step_without_cache_keys_parses_with_none() {
        // A note serialized before the cache fields existed must still parse cleanly,
        // defaulting both to None (no migration). Exercises #[serde(default)] on the new fields.
        let c = parse_check("2026-06-17-0001", RUN).unwrap();
        assert_eq!(c.steps.len(), 2);
        assert!(c.steps[0].cache_read_tokens.is_none());
        assert!(c.steps[0].cache_write_tokens.is_none());
        assert!(c.steps[1].cache_read_tokens.is_none());
        assert!(c.steps[1].cache_write_tokens.is_none());
    }

    #[test]
    fn old_step_without_warnings_key_parses_with_empty_warnings() {
        // A note serialized before the warnings field was added must still parse cleanly.
        // (Exercises the `#[serde(default)]` on Step.warnings.)
        let text = "---\nid: 2026-06-17-0001\nkind: job_check\ntrigger: manual\nstatus: complete\nstarted_at: 2026-06-17T10:00:00\nsubject: stripe\nroles_found: 2\nerrors: 0\nsteps:\n  - stage: careers-scrape\n    class: scrape\n    target: stripe\n    status: ok\n    attempts: 1\n    cost: 5\n---\n\n## Summary\n\njob_check stripe · 2 roles found · 0 errors\n";
        let c = parse_check("2026-06-17-0001", text).unwrap();
        assert_eq!(c.steps.len(), 1);
        assert!(
            c.steps[0].warnings.is_empty(),
            "missing warnings key should default to empty vec"
        );
    }
}
