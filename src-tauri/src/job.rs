//! The `Job` entity (the pipeline's output): parse, derived `jd_fetched`, the
//! new-note render path, and `list_jobs`. Mirrors `company.rs`; uses `crate::note`.

use crate::note::{self, split_frontmatter};
use serde::Serialize;
use std::path::Path;

// These are public API for later tasks (check.rs, pipeline commands) — suppress premature dead_code.
#[allow(dead_code)]
pub const JOB_STATUSES: &[&str] = &["new", "reviewed", "pursuing", "skipped"];

/// Valid machine values for the `level` field. Must stay in sync with the LLM prompt
/// (`prompts.rs`) and the front-end `LEVEL_LABELS` map (`src/lib/level.ts`).
pub const VALID_LEVELS: &[&str] = &[
    "junior", "mid", "senior",
    "front-line-mgmt", "middle-mgmt", "dept-head",
    "vp", "c-suite",
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Job {
    pub slug: String,
    pub title: String,
    /// Bare company slug, unwrapped from the `company: "[[slug]]"` link.
    pub company: Option<String>,
    pub url: Option<String>,
    pub level: Option<String>,
    pub location: Option<String>,
    pub comp_low: Option<i64>,
    pub comp_high: Option<i64>,
    pub comp_currency: Option<String>,
    pub comp_raw: Option<String>,
    pub date_posted: Option<String>,
    pub last_seen: Option<String>,
    pub ats: Option<String>,
    pub tech_stack: Vec<String>,
    pub fit_score: Option<i64>,
    /// new | reviewed | pursuing | skipped
    pub status: Option<String>,
    pub skip_reason: Option<String>,
    pub jd_raw_file: Option<String>,
    /// Derived: a structured JD has been fetched for this role (powers the §4.2 gate).
    pub jd_fetched: bool,
}

#[derive(serde::Deserialize)]
struct Front {
    title: Option<String>,
    company: Option<String>,
    url: Option<String>,
    level: Option<String>,
    location: Option<String>,
    comp_low: Option<i64>,
    comp_high: Option<i64>,
    comp_currency: Option<String>,
    comp_raw: Option<String>,
    date_posted: Option<String>,
    last_seen: Option<String>,
    ats: Option<String>,
    #[serde(default)]
    tech_stack: Vec<String>,
    fit_score: Option<i64>,
    status: Option<String>,
    skip_reason: Option<String>,
    jd_raw_file: Option<String>,
}

/// Unwrap a `[[slug]]` wikilink to its bare slug; pass plain strings through. Trims.
pub fn strip_wikilink(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("[[")
        .trim_end_matches("]]")
        .trim()
        .to_string()
}

#[allow(dead_code)]
pub fn validate_job_status(status: &str) -> Result<(), String> {
    if JOB_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!("unknown job status {status:?}; expected one of {JOB_STATUSES:?}"))
    }
}

pub fn parse_job(slug: &str, text: &str) -> Result<Job, String> {
    let (fm, body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    let jd_fetched = f.jd_raw_file.is_some() || body.contains("## JD — structured");
    Ok(Job {
        slug: slug.to_string(),
        title: f.title.unwrap_or_else(|| slug.to_string()),
        company: f.company.as_deref().map(strip_wikilink),
        url: f.url,
        level: f.level,
        location: f.location,
        comp_low: f.comp_low,
        comp_high: f.comp_high,
        comp_currency: f.comp_currency,
        comp_raw: f.comp_raw,
        date_posted: f.date_posted,
        last_seen: f.last_seen,
        ats: f.ats,
        tech_stack: f.tech_stack,
        fit_score: f.fit_score,
        status: f.status,
        skip_reason: f.skip_reason,
        jd_raw_file: f.jd_raw_file,
        jd_fetched,
    })
}

#[tauri::command]
pub fn list_jobs(vault_path: String) -> Result<Vec<Job>, String> {
    let dir = Path::new(&vault_path).join("jobs");
    let mut out = note::read_notes_in(&dir, parse_job)?;
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

/// Build a job note's text from scalar fields (used by the pipeline to write a stub).
/// Frontmatter via serde_yaml (so titles/URLs with `:` can't corrupt YAML). No body —
/// the `## JD — structured` / `## Alignment analysis` sections are added later via
/// `crate::note::set_body`.
#[allow(dead_code)]
pub fn render_job_note(job: &Job) -> String {
    #[derive(Serialize)]
    struct Fm<'a> {
        id: &'a str,
        title: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        company: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        level: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_low: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_high: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_currency: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_raw: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        date_posted: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_seen: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ats: Option<&'a str>,
        tech_stack: &'a [String], // always emitted (mirrors company.rs domain/business_model)
        #[serde(skip_serializing_if = "Option::is_none")]
        fit_score: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        skip_reason: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        jd_raw_file: Option<&'a str>,
    }
    let fm = Fm {
        id: &job.slug,
        title: &job.title,
        company: job.company.as_ref().map(|c| format!("[[{c}]]")),
        url: job.url.as_deref(),
        level: job.level.as_deref(),
        location: job.location.as_deref(),
        comp_low: job.comp_low,
        comp_high: job.comp_high,
        comp_currency: job.comp_currency.as_deref(),
        comp_raw: job.comp_raw.as_deref(),
        date_posted: job.date_posted.as_deref(),
        last_seen: job.last_seen.as_deref(),
        ats: job.ats.as_deref(),
        tech_stack: &job.tech_stack,
        fit_score: job.fit_score,
        status: job.status.as_deref(),
        skip_reason: job.skip_reason.as_deref(),
        jd_raw_file: job.jd_raw_file.as_deref(),
    };
    let yaml = serde_yaml::to_string(&fm).expect("job frontmatter serializes");
    format!("---\n{yaml}---\n")
}

/// A job stub's slug: `<title>-<company>` slugified (filename == id == slug).
#[allow(dead_code)]
pub fn job_slug(title: &str, company_slug: &str) -> String {
    note::slugify(&format!("{title}-{company_slug}"))
}

/// First free slug among `base`, `base-2`, `base-3`, … per the `taken` predicate. Distinct jobs
/// at one company can share a `<title>-<company>` base slug (different URLs); this hands each a
/// unique, human-readable filename. The bare `base` is preferred; suffixes never renumber, so a
/// slug stays stable once assigned. Terminates: only finitely many slugs are ever `taken`.
fn first_free_slug(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !taken(s))
        .expect("an unused numeric suffix always exists")
}

/// Write a NEW job stub under `<vault>/jobs/`. `job.slug` is the *base*; if that filename is
/// taken, a numeric suffix disambiguates (`-2`, `-3`, …) so two distinct roles that share a
/// `<title>-<company>` slug each get their own note. Returns the slug actually written
/// (filename == id == slug). The URL is a job's identity and dedup-by-url runs upstream
/// (`prefilter`), so reaching here always means a genuinely new note — never a re-find.
#[allow(dead_code)]
pub fn write_job_stub(vault_path: &str, job: &Job) -> Result<String, String> {
    let jobs_dir = Path::new(vault_path).join("jobs");
    let slug = first_free_slug(&job.slug, |s| jobs_dir.join(format!("{s}.md")).exists());
    let mut note = job.clone();
    note.slug = slug.clone(); // the `id` frontmatter must match the disambiguated filename
    note::write_note(&jobs_dir.join(format!("{slug}.md")), &render_job_note(&note))?;
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUB: &str = "---\nid: senior-engineer-stripe\ntitle: \"Senior Engineer\"\ncompany: \"[[stripe]]\"\nurl: https://stripe.com/jobs/123\nlevel: senior\nlocation: Remote (US)\nats: greenhouse\nstatus: new\nlast_seen: 2026-06-17\n---\n\n";

    const FETCHED: &str = "---\nid: head-of-eng-acme\ntitle: \"Head of Engineering\"\ncompany: \"[[acme]]\"\nurl: https://acme.com/jobs/9\nlevel: dept-head\ncomp_low: 200000\ncomp_high: 260000\ncomp_currency: USD\ntech_stack: [\"rust\", \"typescript\"]\nfit_score: 8\nstatus: reviewed\njd_raw_file: _jd/head-of-eng-acme.md\n---\n\n## JD — structured\n\nstuff\n";

    #[test]
    fn parses_stub_and_jd_not_fetched() {
        let j = parse_job("senior-engineer-stripe", STUB).unwrap();
        assert_eq!(j.slug, "senior-engineer-stripe");
        assert_eq!(j.title, "Senior Engineer");
        assert_eq!(j.company.as_deref(), Some("stripe")); // wikilink stripped
        assert_eq!(j.level.as_deref(), Some("senior"));
        assert_eq!(j.status.as_deref(), Some("new"));
        assert_eq!(j.last_seen.as_deref(), Some("2026-06-17"));
        assert!(!j.jd_fetched); // no jd_raw_file and no "## JD — structured"
        assert!(j.tech_stack.is_empty());
    }

    #[test]
    fn derives_jd_fetched_from_raw_file_or_body() {
        let j = parse_job("head-of-eng-acme", FETCHED).unwrap();
        assert!(j.jd_fetched); // jd_raw_file present
        assert_eq!(j.comp_low, Some(200000));
        assert_eq!(j.comp_high, Some(260000));
        assert_eq!(j.fit_score, Some(8));
        assert_eq!(j.tech_stack, vec!["rust".to_string(), "typescript".to_string()]);
    }

    #[test]
    fn jd_fetched_true_from_body_header_alone() {
        let t = "---\nid: x\ntitle: X\n---\n\n## JD — structured\n\nbody\n";
        assert!(parse_job("x", t).unwrap().jd_fetched);
    }

    #[test]
    fn strip_wikilink_unwraps_and_passes_through() {
        assert_eq!(strip_wikilink("[[stripe]]"), "stripe");
        assert_eq!(strip_wikilink("stripe"), "stripe");
        assert_eq!(strip_wikilink("  [[a-b]]  "), "a-b");
    }

    #[test]
    fn list_jobs_reads_dir_skips_underscored_and_missing_dir_empty() {
        let dir = std::env::temp_dir().join(format!("lodestar-job-test-{}", std::process::id()));
        let jobs = dir.join("jobs");
        std::fs::create_dir_all(&jobs).unwrap();
        std::fs::write(jobs.join("senior-engineer-stripe.md"), STUB).unwrap();
        std::fs::write(jobs.join("head-of-eng-acme.md"), FETCHED).unwrap();
        std::fs::write(jobs.join("_template.md"), STUB).unwrap(); // must be skipped
        let vault = dir.to_str().unwrap().to_string();

        let mut list = list_jobs(vault).unwrap();
        list.sort_by(|a, b| a.slug.cmp(&b.slug));
        let slugs: Vec<_> = list.iter().map(|j| j.slug.as_str()).collect();
        assert_eq!(slugs, vec!["head-of-eng-acme", "senior-engineer-stripe"]);

        std::fs::remove_dir_all(&dir).ok();

        // Missing jobs/ dir -> empty list, never an error (no jobs until the pipeline runs).
        assert!(list_jobs("/no/such/vault".to_string()).unwrap().is_empty());
    }

    #[test]
    fn render_round_trips_through_parse() {
        let j = parse_job("head-of-eng-acme", FETCHED).unwrap();
        let text = render_job_note(&j);
        assert!(text.starts_with("---\n"));
        let again = parse_job("head-of-eng-acme", &text).unwrap();
        assert_eq!(again.title, "Head of Engineering");
        assert_eq!(again.company.as_deref(), Some("acme")); // re-wrapped + re-stripped
        assert_eq!(again.comp_low, Some(200000));
        assert_eq!(again.tech_stack, vec!["rust".to_string(), "typescript".to_string()]);
        assert_eq!(again.status.as_deref(), Some("reviewed"));
        assert_eq!(again.jd_raw_file.as_deref(), Some("_jd/head-of-eng-acme.md"));
        assert_eq!(again.level.as_deref(), Some("dept-head"));
    }

    #[test]
    fn job_slug_is_title_plus_company() {
        assert_eq!(
            job_slug("Senior Software Engineer", "stripe"),
            "senior-software-engineer-stripe"
        );
    }

    #[test]
    fn first_free_slug_appends_numeric_suffix_on_collision() {
        // Bare slug when free; otherwise the first free of base-2, base-3, … (never renumbers).
        assert_eq!(first_free_slug("base", |s| ["base", "base-2"].contains(&s)), "base-3");
        assert_eq!(first_free_slug("base", |s| s == "base"), "base-2");
        assert_eq!(first_free_slug("free", |s| ["base", "base-2"].contains(&s)), "free");
    }

    #[test]
    fn write_job_stub_disambiguates_same_title_distinct_jobs() {
        let dir = std::env::temp_dir().join(format!("lodestar-jobstub-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap().to_string();

        // Two genuinely different roles share a title+company (different URLs upstream). Both must
        // get their own note — the second is disambiguated, never silently dropped.
        let j = parse_job("senior-engineer-stripe", STUB).unwrap();
        let slug_a = write_job_stub(&vault, &j).unwrap();
        let slug_b = write_job_stub(&vault, &j).unwrap();
        assert_eq!(slug_a, "senior-engineer-stripe");
        assert_eq!(slug_b, "senior-engineer-stripe-2");
        assert!(dir.join("jobs/senior-engineer-stripe.md").exists());
        assert!(dir.join("jobs/senior-engineer-stripe-2.md").exists());

        // The disambiguated note's `id` matches its filename (filename == id == slug).
        let text_b = std::fs::read_to_string(dir.join("jobs/senior-engineer-stripe-2.md")).unwrap();
        assert!(text_b.contains("id: senior-engineer-stripe-2"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
