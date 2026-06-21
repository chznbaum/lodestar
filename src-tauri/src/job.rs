//! The `Job` entity (the pipeline's output): parse, derived `jd_fetched`, the
//! new-note render path, and `list_jobs`. Mirrors `company.rs`; uses `crate::note`.

use crate::note::{self, split_frontmatter};
use serde::Serialize;
use std::path::Path;

// These are public API for later tasks (check.rs, pipeline commands) — suppress premature dead_code.
#[allow(dead_code)]
pub const JOB_STATUSES: &[&str] = &["new", "reviewed", "pursuing", "skipped"];

// Suppress dead_code until the write helpers (later in this file) start using them.
#[allow(dead_code)]
pub const EMPLOYMENT_TYPES: &[&str] = &[
    "full_time", "part_time", "contract", "fractional", "internship", "temporary",
];

#[allow(dead_code)]
pub const REMOTE_KINDS: &[&str] = &["remote", "hybrid", "onsite"];

/// Valid values for `visa_sponsorship` and `relocation`.
#[allow(dead_code)]
pub const SPONSORSHIP: &[&str] = &["offered", "not_offered", "unspecified"];

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
    // Comp fields
    pub comp_low: Option<i64>,
    pub comp_high: Option<i64>,
    pub comp_currency: Option<String>,
    pub comp_raw: Option<String>,
    /// "annual" | "monthly" | "hourly" etc.
    pub comp_period: Option<String>,
    pub comp_equity: Option<String>,
    // Role classification
    pub employment_type: Option<String>,
    pub yoe_min: Option<i64>,
    pub yoe_max: Option<i64>,
    pub tech_stack: Vec<String>,
    pub required_skills: Vec<String>,
    pub preferred_skills: Vec<String>,
    // Org context
    pub reports_to: Option<String>,
    pub team: Option<String>,
    // Location / logistics
    pub remote: Option<String>,
    pub location_constraints: Option<String>,
    pub visa_sponsorship: Option<String>,
    pub relocation: Option<String>,
    pub countries: Vec<String>,
    pub metros: Vec<String>,
    pub application_url: Option<String>,
    // Pipeline metadata
    pub date_posted: Option<String>,
    pub last_seen: Option<String>,
    pub ats: Option<String>,
    pub fit_score: Option<i64>,
    /// Fields filled by the `research-gaps` stage — recorded for provenance.
    pub researched: Vec<String>,
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
    // Comp fields
    comp_low: Option<i64>,
    comp_high: Option<i64>,
    comp_currency: Option<String>,
    comp_raw: Option<String>,
    comp_period: Option<String>,
    comp_equity: Option<String>,
    // Role classification
    employment_type: Option<String>,
    yoe_min: Option<i64>,
    yoe_max: Option<i64>,
    #[serde(default)]
    tech_stack: Vec<String>,
    #[serde(default)]
    required_skills: Vec<String>,
    #[serde(default)]
    preferred_skills: Vec<String>,
    // Org context
    reports_to: Option<String>,
    team: Option<String>,
    // Location / logistics
    remote: Option<String>,
    location_constraints: Option<String>,
    visa_sponsorship: Option<String>,
    relocation: Option<String>,
    #[serde(default)]
    countries: Vec<String>,
    #[serde(default)]
    metros: Vec<String>,
    application_url: Option<String>,
    // Pipeline metadata
    date_posted: Option<String>,
    last_seen: Option<String>,
    ats: Option<String>,
    fit_score: Option<i64>,
    #[serde(default)]
    researched: Vec<String>,
    status: Option<String>,
    skip_reason: Option<String>,
    jd_raw_file: Option<String>,
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
    let f: Front = parse_front_lenient(slug, fm)?;
    let jd_fetched = f.jd_raw_file.is_some() || body.contains("## JD — structured");
    Ok(Job {
        slug: slug.to_string(),
        title: f.title.unwrap_or_else(|| slug.to_string()),
        company: f.company.as_deref().map(note::strip_wikilink),
        url: f.url,
        level: f.level,
        location: f.location,
        comp_low: f.comp_low,
        comp_high: f.comp_high,
        comp_currency: f.comp_currency,
        comp_raw: f.comp_raw,
        comp_period: f.comp_period,
        comp_equity: f.comp_equity,
        employment_type: f.employment_type,
        yoe_min: f.yoe_min,
        yoe_max: f.yoe_max,
        tech_stack: f.tech_stack,
        required_skills: f.required_skills,
        preferred_skills: f.preferred_skills,
        reports_to: f.reports_to,
        team: f.team,
        remote: f.remote,
        location_constraints: f.location_constraints,
        visa_sponsorship: f.visa_sponsorship,
        relocation: f.relocation,
        countries: f.countries,
        metros: f.metros,
        application_url: f.application_url,
        date_posted: f.date_posted,
        last_seen: f.last_seen,
        ats: f.ats,
        fit_score: f.fit_score,
        researched: f.researched,
        status: f.status,
        skip_reason: f.skip_reason,
        jd_raw_file: f.jd_raw_file,
        jd_fetched,
    })
}

/// Integer-typed frontmatter fields — validated on write, coerced/degraded on read.
#[allow(dead_code)]
const INT_FIELDS: &[&str] = &["comp_low", "comp_high", "yoe_min", "yoe_max", "fit_score"];
/// List-typed frontmatter fields — set via `set_job_list_field`, never the scalar writer.
#[allow(dead_code)]
const LIST_FIELDS: &[&str] = &["tech_stack", "required_skills", "preferred_skills", "researched", "countries", "metros"];

/// Deserialize the frontmatter strictly; on failure, sanitize the typed (int/list) fields so a
/// single malformed value degrades to empty (with a logged warning) and retry — one bad field
/// never makes the whole note vanish. A note that's still unparseable returns the original error.
fn parse_front_lenient(slug: &str, fm: &str) -> Result<Front, String> {
    let orig = match serde_yaml::from_str::<Front>(fm) {
        Ok(f) => return Ok(f),
        Err(e) => e,
    };
    let mut value: serde_yaml::Value =
        serde_yaml::from_str(fm).map_err(|_| format!("{slug}: {orig}"))?;
    let serde_yaml::Value::Mapping(map) = &mut value else {
        return Err(format!("{slug}: {orig}"));
    };
    let warnings = note::sanitize_typed_fields(map, INT_FIELDS, LIST_FIELDS);
    let f = serde_yaml::from_value::<Front>(value).map_err(|_| format!("{slug}: {orig}"))?;
    for w in &warnings {
        eprintln!("{slug}: {w}");
    }
    Ok(f)
}

#[tauri::command]
pub fn list_jobs(vault_path: String) -> Result<Vec<Job>, String> {
    let dir = Path::new(&vault_path).join("jobs");
    let mut out = note::read_notes_in(&dir, parse_job)?;
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

/// `skip_serializing_if` helper for `&[String]` fields in the `Fm` struct — serde passes
/// `&&[String]` to the predicate so `Vec::is_empty` doesn't match the type.
fn slice_is_empty(v: &&[String]) -> bool {
    v.is_empty()
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
        // Comp fields
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_low: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_high: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_currency: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_raw: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_period: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comp_equity: Option<&'a str>,
        // Role classification
        #[serde(skip_serializing_if = "Option::is_none")]
        employment_type: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        yoe_min: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        yoe_max: Option<i64>,
        tech_stack: &'a [String], // always emitted (mirrors company.rs domain/business_model)
        #[serde(skip_serializing_if = "slice_is_empty")]
        required_skills: &'a [String],
        #[serde(skip_serializing_if = "slice_is_empty")]
        preferred_skills: &'a [String],
        // Org context
        #[serde(skip_serializing_if = "Option::is_none")]
        reports_to: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        team: Option<&'a str>,
        // Location / logistics
        #[serde(skip_serializing_if = "Option::is_none")]
        remote: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        location_constraints: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        visa_sponsorship: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        relocation: Option<&'a str>,
        #[serde(skip_serializing_if = "slice_is_empty")]
        countries: &'a [String],
        #[serde(skip_serializing_if = "slice_is_empty")]
        metros: &'a [String],
        #[serde(skip_serializing_if = "Option::is_none")]
        application_url: Option<&'a str>,
        // Pipeline metadata
        #[serde(skip_serializing_if = "Option::is_none")]
        date_posted: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_seen: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ats: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fit_score: Option<i64>,
        /// Populated fields from the research-gaps stage; omit when empty.
        #[serde(skip_serializing_if = "slice_is_empty")]
        researched: &'a [String],
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
        comp_period: job.comp_period.as_deref(),
        comp_equity: job.comp_equity.as_deref(),
        employment_type: job.employment_type.as_deref(),
        yoe_min: job.yoe_min,
        yoe_max: job.yoe_max,
        tech_stack: &job.tech_stack,
        required_skills: &job.required_skills,
        preferred_skills: &job.preferred_skills,
        reports_to: job.reports_to.as_deref(),
        team: job.team.as_deref(),
        remote: job.remote.as_deref(),
        location_constraints: job.location_constraints.as_deref(),
        visa_sponsorship: job.visa_sponsorship.as_deref(),
        relocation: job.relocation.as_deref(),
        countries: &job.countries,
        metros: &job.metros,
        application_url: job.application_url.as_deref(),
        date_posted: job.date_posted.as_deref(),
        last_seen: job.last_seen.as_deref(),
        ats: job.ats.as_deref(),
        fit_score: job.fit_score,
        researched: &job.researched,
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

/// Map an enum-validated field name to its allowed value set. Other fields are free text.
#[allow(dead_code)]
fn enum_values_for(field: &str) -> Option<&'static [&'static str]> {
    match field {
        "level" => Some(VALID_LEVELS),
        "employment_type" => Some(EMPLOYMENT_TYPES),
        "remote" => Some(REMOTE_KINDS),
        "visa_sponsorship" | "relocation" => Some(SPONSORSHIP),
        "status" => Some(JOB_STATUSES),
        _ => None,
    }
}

/// Type-aware field-level frontmatter write. Validates and YAML-safe-encodes `value` per the
/// field's type so arbitrary (e.g. LLM-produced) input can't corrupt the note: enum fields reject
/// out-of-set values; integer fields reject non-numeric input; every other field is written as a
/// quoted-when-needed scalar. List fields must use `set_job_list_field`. An empty value clears the
/// field. The write lands via the single choke point.
#[allow(dead_code)]
#[tauri::command]
pub fn update_job_field(
    vault_path: String,
    slug: String,
    field: String,
    value: String,
) -> Result<(), String> {
    if LIST_FIELDS.contains(&field.as_str()) {
        return Err(format!("{field} is a list field; use set_job_list_field"));
    }
    let fragment = if value.is_empty() {
        String::new() // clears the field (serialized as `field:` → null → None)
    } else if let Some(allowed) = enum_values_for(&field) {
        if !allowed.contains(&value.as_str()) {
            return Err(format!(
                "invalid {field} value {value:?}; expected one of {allowed:?}"
            ));
        }
        note::yaml_scalar(&value)?
    } else if INT_FIELDS.contains(&field.as_str()) {
        let n: i64 = value
            .trim()
            .parse()
            .map_err(|_| format!("{field} expects an integer, got {value:?}"))?;
        n.to_string()
    } else {
        note::yaml_scalar(&value)?
    };
    let path = Path::new(&vault_path)
        .join("jobs")
        .join(format!("{slug}.md"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    let updated = note::set_frontmatter_field(&text, &field, &fragment)?;
    note::write_note(&path, &updated)
}

/// Set a list-typed job field (tech_stack/required_skills/preferred_skills/researched) to `values`,
/// written as a YAML-safe flow sequence so items with commas/colons/quotes round-trip exactly.
/// Rejects non-list fields. An empty `values` writes `[]` (clears the list).
#[allow(dead_code)]
#[tauri::command]
pub fn set_job_list_field(
    vault_path: String,
    slug: String,
    field: String,
    values: Vec<String>,
) -> Result<(), String> {
    if !LIST_FIELDS.contains(&field.as_str()) {
        return Err(format!("{field} is not a list field"));
    }
    let fragment = note::yaml_flow_seq(&values)?;
    let path = Path::new(&vault_path)
        .join("jobs")
        .join(format!("{slug}.md"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    let updated = note::set_frontmatter_field(&text, &field, &fragment)?;
    note::write_note(&path, &updated)
}

/// Set the job's status (validated) and optionally record a skip_reason.
#[allow(dead_code)]
#[tauri::command]
pub fn set_job_status(
    vault_path: String,
    slug: String,
    status: String,
    skip_reason: Option<String>,
) -> Result<(), String> {
    validate_job_status(&status)?;
    update_job_field(vault_path.clone(), slug.clone(), "status".into(), status)?;
    if let Some(r) = skip_reason {
        update_job_field(vault_path, slug, "skip_reason".into(), r)?;
    }
    Ok(())
}

/// Insert-or-replace a single `## heading` section in the job note's body, leaving frontmatter
/// and all other sections untouched. `heading` must include the leading `## `.
#[allow(dead_code)]
pub fn set_job_section(
    vault_path: &str,
    slug: &str,
    heading: &str,
    markdown: &str,
) -> Result<(), String> {
    let path = Path::new(vault_path)
        .join("jobs")
        .join(format!("{slug}.md"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {path:?}: {e}"))?;
    let (_fm, body) = note::split_frontmatter(&text);
    let new_section = format!("{heading}\n\n{}\n", markdown.trim());
    let new_body = upsert_section(body, heading.trim(), &new_section);
    let updated = note::set_body(&text, &new_body)?;
    note::write_note(&path, &updated)
}

/// Rebuild `body` with the `## ` section whose heading equals `target` (trimmed) replaced by
/// `new_section`, or `new_section` appended if no such section exists. Every other section — and
/// any preamble before the first heading — is preserved. `new_section` is the full replacement
/// text including its own heading line. Sections are re-joined with a single blank-line separator.
#[allow(dead_code)]
fn upsert_section(body: &str, target: &str, new_section: &str) -> String {
    // Partition into chunks, each beginning at a `## ` heading. Lines before the first heading
    // form a leading preamble chunk so nothing is dropped.
    let mut chunks: Vec<String> = Vec::new();
    for line in body.lines() {
        if line.starts_with("## ") || chunks.is_empty() {
            chunks.push(String::new());
        }
        let chunk = chunks.last_mut().expect("chunk pushed above");
        chunk.push_str(line);
        chunk.push('\n');
    }

    // Replace the first chunk whose heading matches; otherwise append.
    let mut replaced = false;
    for chunk in &mut chunks {
        let heading = chunk.lines().next().unwrap_or("");
        if heading.trim() == target {
            *chunk = new_section.to_string();
            replaced = true;
            break;
        }
    }
    if !replaced {
        chunks.push(new_section.to_string());
    }

    chunks
        .iter()
        .map(|c| c.trim_end())
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUB: &str = "---\nid: senior-engineer-stripe\ntitle: \"Senior Engineer\"\ncompany: \"[[stripe]]\"\nurl: https://stripe.com/jobs/123\nlevel: senior\nlocation: Remote (US)\nats: greenhouse\nstatus: new\nlast_seen: 2026-06-17\n---\n\n";

    const DETAIL: &str = "---\nid: senior-engineer-acme\ntitle: \"Senior Engineer\"\ncompany: \"[[acme]]\"\nurl: https://acme.com/j/1\nlevel: senior\ncomp_low: 180000\ncomp_high: 220000\ncomp_currency: USD\ncomp_period: annual\ncomp_equity: \"0.1-0.4%\"\nemployment_type: full_time\nyoe_min: 5\nyoe_max: 8\ntech_stack: [\"rust\"]\nrequired_skills: [\"rust\", \"distributed-systems\"]\npreferred_skills: [\"kubernetes\"]\nreports_to: CTO\nteam: Platform\nremote: remote\nlocation_constraints: \"US only\"\nvisa_sponsorship: not_offered\nrelocation: unspecified\napplication_url: https://acme.com/apply/1\nfit_score: 72\nresearched: [\"comp_low\", \"comp_high\"]\nstatus: reviewed\n---\n\n## JD — structured\n\nbody\n";

    #[test]
    fn parses_and_renders_all_jd_detail_fields() {
        let j = parse_job("senior-engineer-acme", DETAIL).unwrap();
        assert_eq!(j.comp_period.as_deref(), Some("annual"));
        assert_eq!(j.employment_type.as_deref(), Some("full_time"));
        assert_eq!(j.yoe_min, Some(5));
        assert_eq!(j.required_skills, vec!["rust", "distributed-systems"]);
        assert_eq!(j.remote.as_deref(), Some("remote"));
        assert_eq!(j.visa_sponsorship.as_deref(), Some("not_offered"));
        assert_eq!(j.researched, vec!["comp_low", "comp_high"]);
        // round-trip preserves them
        let again = parse_job("senior-engineer-acme", &render_job_note(&j)).unwrap();
        assert_eq!(again.required_skills, j.required_skills);
        assert_eq!(again.relocation.as_deref(), Some("unspecified"));
        assert_eq!(again.fit_score, Some(72));
    }

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

    #[test]
    fn update_job_field_validates_enums_and_writes() {
        let dir = std::env::temp_dir().join(format!("lodestar-jobwrite-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        std::fs::write(dir.join("jobs/senior-engineer-acme.md"), DETAIL).unwrap();

        // good value persists
        update_job_field(vault.clone(), "senior-engineer-acme".into(), "remote".into(), "hybrid".into()).unwrap();
        let j = parse_job("senior-engineer-acme",
            &std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap()).unwrap();
        assert_eq!(j.remote.as_deref(), Some("hybrid"));

        // bad enum value rejected, file unchanged
        assert!(update_job_field(vault.clone(), "senior-engineer-acme".into(), "employment_type".into(), "wizard".into()).is_err());

        // status + skip_reason
        set_job_status(vault.clone(), "senior-engineer-acme".into(), "skipped".into(), Some("comp below floor".into())).unwrap();
        let j2 = parse_job("senior-engineer-acme",
            &std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap()).unwrap();
        assert_eq!(j2.status.as_deref(), Some("skipped"));
        assert_eq!(j2.skip_reason.as_deref(), Some("comp below floor"));

        // body section upsert preserves frontmatter + adds the heading
        set_job_section(&vault, "senior-engineer-acme", "## Alignment analysis", "Strong fit.").unwrap();
        let txt = std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap();
        assert!(txt.contains("## Alignment analysis"));
        assert!(txt.contains("Strong fit."));
        assert!(txt.contains("title:")); // frontmatter intact
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_job_section_preserves_sibling_sections() {
        let dir = std::env::temp_dir().join(format!("lodestar-jobsec-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap();
        let start = "---\nid: x\ntitle: X\n---\n\n## JD — structured\n\njd text\n\n## Outreach notes\n\nping someone\n";
        std::fs::write(dir.join("jobs/x.md"), start).unwrap();
        set_job_section(vault, "x", "## Alignment analysis", "fits well").unwrap();
        let t = std::fs::read_to_string(dir.join("jobs/x.md")).unwrap();
        assert!(t.contains("## JD — structured") && t.contains("jd text"));
        assert!(t.contains("## Outreach notes") && t.contains("ping someone"));
        assert!(t.contains("## Alignment analysis") && t.contains("fits well"));
        // re-running replaces, not duplicates
        set_job_section(vault, "x", "## Alignment analysis", "fits very well").unwrap();
        let t2 = std::fs::read_to_string(dir.join("jobs/x.md")).unwrap();
        assert_eq!(t2.matches("## Alignment analysis").count(), 1);
        assert!(t2.contains("fits very well") && !t2.contains("fits well\n"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_job_section_replaces_middle_section_cleanly() {
        let dir = std::env::temp_dir().join(format!("lodestar-jobmid-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap();
        // The replaced section ("## JD — structured") has siblings AFTER it — the realistic
        // re-run case where structure-jd reruns after research-gaps/alignment already appended.
        let start = "---\nid: y\ntitle: Y\n---\n\n## JD — structured\n\nold jd\n\n## Research notes\n\nnotes\n\n## Alignment analysis\n\nfits\n";
        std::fs::write(dir.join("jobs/y.md"), start).unwrap();
        set_job_section(vault, "y", "## JD — structured", "new jd").unwrap();
        let t = std::fs::read_to_string(dir.join("jobs/y.md")).unwrap();
        // Replacement landed, exactly once, no stale content.
        assert_eq!(t.matches("## JD — structured").count(), 1);
        assert!(t.contains("new jd") && !t.contains("old jd"));
        // Siblings below the replaced section survive intact.
        assert!(t.contains("## Research notes") && t.contains("notes"));
        assert!(t.contains("## Alignment analysis") && t.contains("fits"));
        // No internal sentinel ever reaches the note body.
        assert!(!t.contains('\u{0}'), "sentinel leaked into note body:\n{t}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_job_field_validates_and_safely_encodes_by_type() {
        let dir = std::env::temp_dir().join(format!("lodestar-jobtyped-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        std::fs::write(dir.join("jobs/senior-engineer-acme.md"), DETAIL).unwrap();
        let read = || {
            parse_job(
                "senior-engineer-acme",
                &std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap(),
            )
            .unwrap()
        };

        // Free text with YAML-special chars round-trips EXACTLY (the corruption case).
        let tricky = "US only; sponsorship: no, prefers \"west coast\" [note]";
        update_job_field(vault.clone(), "senior-engineer-acme".into(), "location_constraints".into(), tricky.into()).unwrap();
        assert_eq!(read().location_constraints.as_deref(), Some(tricky));

        // Integer field: valid persists; non-numeric is rejected and the file is unchanged.
        update_job_field(vault.clone(), "senior-engineer-acme".into(), "comp_low".into(), "150000".into()).unwrap();
        assert_eq!(read().comp_low, Some(150000));
        assert!(update_job_field(vault.clone(), "senior-engineer-acme".into(), "comp_low".into(), "lots".into()).is_err());
        assert_eq!(read().comp_low, Some(150000)); // rejected write left it untouched

        // List field can't be set through the scalar writer.
        assert!(update_job_field(vault.clone(), "senior-engineer-acme".into(), "required_skills".into(), "rust".into()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_job_list_field_round_trips_special_items() {
        let dir = std::env::temp_dir().join(format!("lodestar-joblist-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        std::fs::write(dir.join("jobs/senior-engineer-acme.md"), DETAIL).unwrap();
        let items = vec!["rust".to_string(), "distributed systems".to_string(), "a, b".to_string()];
        set_job_list_field(vault.clone(), "senior-engineer-acme".into(), "required_skills".into(), items.clone()).unwrap();
        let j = parse_job(
            "senior-engineer-acme",
            &std::fs::read_to_string(dir.join("jobs/senior-engineer-acme.md")).unwrap(),
        )
        .unwrap();
        assert_eq!(j.required_skills, items);
        // A non-list field is rejected.
        assert!(set_job_list_field(vault.clone(), "senior-engineer-acme".into(), "title".into(), vec!["x".into()]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn location_fields_round_trip() {
        // A job with both countries and metros parses and render→re-parses preserving both.
        let text = "---\nid: eng-acme\ntitle: Engineer\ncompany: \"[[acme]]\"\ncountries: [\"US\"]\nmetros: [\"washington-arlington-alexandria-dc-va-md-wv\"]\n---\n\n";
        let j = parse_job("eng-acme", text).unwrap();
        assert_eq!(j.countries, vec!["US"]);
        assert_eq!(j.metros, vec!["washington-arlington-alexandria-dc-va-md-wv"]);
        // round-trip
        let rendered = render_job_note(&j);
        let again = parse_job("eng-acme", &rendered).unwrap();
        assert_eq!(again.countries, vec!["US"]);
        assert_eq!(again.metros, vec!["washington-arlington-alexandria-dc-va-md-wv"]);
    }

    #[test]
    fn location_fields_default_to_empty_when_absent() {
        // A job note without countries/metros keys parses to empty vecs (serde(default)).
        let text = "---\nid: eng-acme\ntitle: Engineer\n---\n\n";
        let j = parse_job("eng-acme", text).unwrap();
        assert!(j.countries.is_empty());
        assert!(j.metros.is_empty());
    }

    #[test]
    fn parse_job_degrades_bad_typed_fields_instead_of_failing() {
        // comp_low is non-numeric and required_skills is a scalar (not a list): both degrade to
        // empty and the note still loads — it must not vanish over one malformed field.
        let bad = "---\nid: x\ntitle: X\ncomp_low: lots\ncomp_high: 200000\nrequired_skills: rust\nremote: remote\n---\n\nbody\n";
        let j = parse_job("x", bad).unwrap();
        assert_eq!(j.comp_low, None);
        assert_eq!(j.comp_high, Some(200000));
        assert!(j.required_skills.is_empty());
        assert_eq!(j.remote.as_deref(), Some("remote"));
        assert_eq!(j.title, "X");
    }
}
