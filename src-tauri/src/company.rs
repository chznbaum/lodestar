//! The `Company` entity: parsing, derived fields (due-for-check, screening), and all
//! company commands. Uses `crate::note` for generic frontmatter/body I/O.

use crate::note::{self, set_body, set_frontmatter_field, slugify, split_frontmatter};
use chrono::{Local, NaiveDate};
use serde::Serialize;
use std::path::{Path, PathBuf};

const DEALBREAKER: &[&str] = &[
    "defense_military",
    "alcohol",
    "tobacco_vaping",
    "firearms_weapons",
    "gambling",
    "crypto_web3",
    "oil_gas",
];
const CAUTION: &[&str] = &["adult_content"];
pub const STATUSES: &[&str] = &["active", "paused", "exhausted", "removed"];

#[derive(Debug, Serialize, PartialEq)]
pub struct Company {
    pub slug: String,
    pub name: String,
    pub domain: Vec<String>,
    pub business_model: Vec<String>,
    pub status: Option<String>,
    pub remote_policy: Option<String>,
    pub company_size: Option<String>,
    pub stage: Option<String>,
    pub location: Option<String>,
    pub website: Option<String>,
    pub careers_url: Option<String>,
    pub last_checked: Option<String>,
    pub domain_raw: Option<String>,
    pub source: Option<String>,
    pub due_for_check: bool,
    /// "dealbreaker" | "caution" | None
    pub screening: Option<String>,
    pub notes: String,
}

#[derive(serde::Deserialize)]
struct Front {
    name: Option<String>,
    #[serde(default)]
    domain: Vec<String>,
    #[serde(default)]
    business_model: Vec<String>,
    status: Option<String>,
    remote_policy: Option<String>,
    company_size: Option<String>,
    stage: Option<String>,
    location: Option<String>,
    website: Option<String>,
    careers_url: Option<String>,
    last_checked: Option<String>,
    domain_raw: Option<String>,
    source: Option<String>,
}

/// Payload for creating a company from the UI (manual form or web-research auto-fill).
#[derive(serde::Deserialize)]
pub struct NewCompany {
    pub name: String,
    pub website: Option<String>,
    pub careers_url: Option<String>,
    #[serde(default)]
    pub domain: Vec<String>,
    #[serde(default)]
    pub business_model: Vec<String>,
    pub domain_raw: Option<String>,
    pub company_size: Option<String>,
    pub stage: Option<String>,
    pub remote_policy: Option<String>,
    pub location: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub notes: String,
}

/// active AND (never checked OR last check > 3 days before `today`). Unparseable dates => not due.
pub fn is_due(status: Option<&str>, last_checked: Option<&str>, today: NaiveDate) -> bool {
    if status != Some("active") {
        return false;
    }
    match last_checked.map(str::trim).filter(|s| !s.is_empty()) {
        None => true,
        Some(s) => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) => (today - d).num_days() > 3,
            Err(_) => false,
        },
    }
}

pub fn screening_for(domain: &[String]) -> Option<String> {
    if domain.iter().any(|d| DEALBREAKER.contains(&d.as_str())) {
        Some("dealbreaker".into())
    } else if domain.iter().any(|d| CAUTION.contains(&d.as_str())) {
        Some("caution".into())
    } else {
        None
    }
}

pub fn validate_status(status: &str) -> Result<(), String> {
    if STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!("unknown status {status:?}; expected one of {STATUSES:?}"))
    }
}

pub fn parse_company(slug: &str, text: &str, today: NaiveDate) -> Result<Company, String> {
    let (fm, body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    let last_checked = f.last_checked.filter(|s| !s.trim().is_empty());
    let due_for_check = is_due(f.status.as_deref(), last_checked.as_deref(), today);
    let screening = screening_for(&f.domain);
    Ok(Company {
        slug: slug.to_string(),
        name: f.name.unwrap_or_else(|| slug.to_string()),
        domain: f.domain,
        business_model: f.business_model,
        status: f.status,
        remote_policy: f.remote_policy,
        company_size: f.company_size,
        stage: f.stage,
        location: f.location,
        website: f.website,
        careers_url: f.careers_url,
        last_checked,
        domain_raw: f.domain_raw,
        source: f.source,
        due_for_check,
        screening,
        notes: body.trim().to_string(),
    })
}

/// Build a complete note for a new company. Frontmatter is serialized with `serde_yaml`
/// (so names/URLs with `:` or quotes can't corrupt the YAML); body is the notes under `## Notes`.
pub fn render_company_note(slug: &str, nc: &NewCompany) -> String {
    #[derive(Serialize)]
    struct Fm<'a> {
        id: &'a str,
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        website: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        careers_url: Option<&'a str>,
        domain: &'a [String],
        business_model: &'a [String],
        #[serde(skip_serializing_if = "Option::is_none")]
        domain_raw: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        company_size: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stage: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        remote_policy: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<&'a str>,
        status: &'a str,
        last_checked: Option<&'a str>, // always emitted (None -> `null`)
    }
    let fm = Fm {
        id: slug,
        name: &nc.name,
        website: nc.website.as_deref(),
        careers_url: nc.careers_url.as_deref(),
        domain: &nc.domain,
        business_model: &nc.business_model,
        domain_raw: nc.domain_raw.as_deref(),
        company_size: nc.company_size.as_deref(),
        stage: nc.stage.as_deref(),
        remote_policy: nc.remote_policy.as_deref(),
        location: nc.location.as_deref(),
        source: nc.source.as_deref(),
        status: "active",
        last_checked: None,
    };
    let yaml = serde_yaml::to_string(&fm).expect("company frontmatter serializes");
    let body = nc.notes.trim();
    format!("---\n{yaml}---\n\n## Notes\n\n{body}\n")
}

fn company_path(vault_path: &str, slug: &str) -> Result<PathBuf, String> {
    if slug.is_empty() || slug.contains('/') || slug.contains('\\') || slug.starts_with('.') {
        return Err(format!("invalid slug {slug:?}"));
    }
    Ok(Path::new(vault_path)
        .join("companies")
        .join(format!("{slug}.md")))
}

#[tauri::command]
pub fn list_companies(vault_path: String) -> Result<Vec<Company>, String> {
    let today = Local::now().date_naive();
    let dir = Path::new(&vault_path).join("companies");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("read {dir:?}: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let Some(slug) = note::note_slug(file_name) else {
            continue;
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match parse_company(&slug, &text, today) {
            Ok(c) => out.push(c),
            Err(e) => eprintln!("skip {slug}: {e}"),
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn update_company_field(
    vault_path: String,
    slug: String,
    key: String,
    value: String,
) -> Result<Company, String> {
    if key == "id" {
        return Err("refusing to modify the identity field `id`".into());
    }
    let p = company_path(&vault_path, &slug)?;
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let updated = set_frontmatter_field(&text, &key, &value)?;
    std::fs::write(&p, &updated).map_err(|e| e.to_string())?;
    parse_company(&slug, &updated, Local::now().date_naive())
}

#[tauri::command]
pub fn set_company_notes(
    vault_path: String,
    slug: String,
    body: String,
) -> Result<Company, String> {
    let p = company_path(&vault_path, &slug)?;
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let updated = set_body(&text, &body)?;
    std::fs::write(&p, &updated).map_err(|e| e.to_string())?;
    parse_company(&slug, &updated, Local::now().date_naive())
}

/// Soft-remove / retire: a validated `status` write (the UI never hard-deletes notes).
#[tauri::command]
pub fn set_company_status(
    vault_path: String,
    slug: String,
    status: String,
) -> Result<Company, String> {
    validate_status(&status)?;
    update_company_field(vault_path, slug, "status".into(), status)
}

/// Create a new company note from a UI payload. Slug is derived from the name; errors if
/// the name yields an empty slug or a note already exists at that slug.
#[tauri::command]
pub fn create_company(vault_path: String, company: NewCompany) -> Result<Company, String> {
    let slug = slugify(&company.name);
    if slug.is_empty() {
        return Err(format!("name {:?} produced an empty slug", company.name));
    }
    let p = company_path(&vault_path, &slug)?;
    if p.exists() {
        return Err(format!("a company note already exists at {slug:?}"));
    }
    let text = render_company_note(&slug, &company);
    std::fs::write(&p, &text).map_err(|e| e.to_string())?;
    parse_company(&slug, &text, Local::now().date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    const SAMPLE: &str = "---\nid: stripe\nname: \"Stripe\"\ncareers_url: \"https://stripe.com/jobs\"\ndomain: [financial_services]\ncompany_size: enterprise\nstage: series_c_plus\nremote_policy: remote_first\nstatus: active\nlast_checked:\n---\n\n## Notes\n\nDoes not require degrees.\n";

    #[test]
    fn parses_company_fields_and_body() {
        let c = parse_company("stripe", SAMPLE, d("2026-06-15")).unwrap();
        assert_eq!(c.slug, "stripe");
        assert_eq!(c.name, "Stripe");
        assert_eq!(c.domain, vec!["financial_services".to_string()]);
        assert_eq!(c.status.as_deref(), Some("active"));
        assert_eq!(c.careers_url.as_deref(), Some("https://stripe.com/jobs"));
        assert!(c.notes.contains("Does not require degrees."));
    }

    #[test]
    fn active_and_never_checked_is_due() {
        let c = parse_company("stripe", SAMPLE, d("2026-06-15")).unwrap();
        assert!(c.due_for_check);
    }

    #[test]
    fn due_when_active_and_checked_over_3_days_ago() {
        let today = d("2026-06-15");
        assert!(is_due(Some("active"), Some("2026-06-10"), today)); // 5 days
        assert!(!is_due(Some("active"), Some("2026-06-13"), today)); // 2 days
        assert!(is_due(Some("active"), None, today)); // never checked
        assert!(!is_due(Some("paused"), None, today)); // not active
        assert!(!is_due(Some("active"), Some("garbage"), today)); // unparseable -> not due
        assert!(!is_due(Some("active"), Some("2026-06-12"), today)); // exactly 3 days -> not due
    }

    #[test]
    fn screening_flags_dealbreaker_then_caution() {
        assert_eq!(
            screening_for(&["defense_military".into()]).as_deref(),
            Some("dealbreaker")
        );
        assert_eq!(
            screening_for(&["healthcare".into(), "adult_content".into()]).as_deref(),
            Some("caution")
        );
        assert_eq!(screening_for(&["healthcare".into()]), None);
    }

    #[test]
    fn ignores_unknown_frontmatter_keys() {
        let text = "---\nid: x\nname: \"X\"\nstatus: paused\nbusiness_model: [b2b]\ndomain_raw: \"whatever\"\nsource: \"claude_research_batch_1\"\n---\n\nbody\n";
        let c = parse_company("x", text, d("2026-06-15")).unwrap();
        assert_eq!(c.name, "X");
        assert_eq!(c.status.as_deref(), Some("paused"));
        assert_eq!(c.source.as_deref(), Some("claude_research_batch_1"));
        assert!(!c.due_for_check); // not active
    }

    #[test]
    fn validate_status_accepts_known_rejects_unknown() {
        assert!(validate_status("removed").is_ok());
        assert!(validate_status("active").is_ok());
        assert!(validate_status("banana").is_err());
    }

    #[test]
    fn rendered_new_company_roundtrips_through_parse() {
        let nc = NewCompany {
            name: "Acme, Inc.".into(),
            website: Some("https://acme.example".into()),
            careers_url: Some("https://acme.example/careers".into()),
            domain: vec!["fintech".into(), "ai".into()],
            business_model: vec!["b2b".into()],
            domain_raw: Some("payments + ML".into()),
            company_size: Some("scaleup".into()),
            stage: Some("series_b".into()),
            remote_policy: Some("remote_first".into()),
            location: Some("Remote, US".into()),
            source: Some("manual".into()),
            notes: "Why listed: warm intro via X.".into(),
        };
        let text = render_company_note("acme-inc", &nc);
        let c = parse_company("acme-inc", &text, d("2026-06-16")).unwrap();
        assert_eq!(c.name, "Acme, Inc.");
        assert_eq!(c.domain, vec!["fintech".to_string(), "ai".to_string()]);
        assert_eq!(c.business_model, vec!["b2b".to_string()]);
        assert_eq!(c.company_size.as_deref(), Some("scaleup"));
        assert_eq!(c.status.as_deref(), Some("active"));
        assert!(c.last_checked.is_none());
        assert!(c.due_for_check);
        assert!(c.notes.contains("warm intro"));
    }

    #[test]
    fn create_company_writes_then_rejects_duplicate() {
        // unique temp vault dir, no external deps
        let dir = std::env::temp_dir().join(format!("lodestar-test-{}", std::process::id()));
        let companies = dir.join("companies");
        std::fs::create_dir_all(&companies).unwrap();
        let vault = dir.to_str().unwrap().to_string();

        let nc = NewCompany {
            name: "Beta Co".into(),
            website: None,
            careers_url: None,
            domain: vec!["devtools".into()],
            business_model: vec![],
            domain_raw: None,
            company_size: None,
            stage: None,
            remote_policy: None,
            location: None,
            source: None,
            notes: String::new(),
        };
        let c = create_company(vault.clone(), nc).unwrap();
        assert_eq!(c.slug, "beta-co");
        assert!(companies.join("beta-co.md").exists());

        // second create at the same slug must error, not overwrite
        let dup = NewCompany {
            name: "Beta Co".into(),
            website: None, careers_url: None, domain: vec![], business_model: vec![],
            domain_raw: None, company_size: None, stage: None, remote_policy: None,
            location: None, source: None, notes: String::new(),
        };
        assert!(create_company(vault, dup).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn company_path_rejects_traversal_and_separators() {
        assert!(company_path("/vault", "../evil").is_err());
        assert!(company_path("/vault", "a/b").is_err());
        assert!(company_path("/vault", ".hidden").is_err());
        assert!(company_path("/vault", "").is_err());
        assert!(company_path("/vault", "stripe").is_ok());
    }

    #[test]
    fn update_field_refuses_to_change_id() {
        // errors on the id guard before any filesystem access
        let r = update_company_field("/vault".into(), "stripe".into(), "id".into(), "x".into());
        assert!(r.is_err());
    }

    #[test]
    #[ignore = "reads the real vault; run with LODESTAR_VAULT=<path> cargo test -- --ignored --nocapture"]
    fn smoke_parses_real_vault() {
        let path = std::env::var("LODESTAR_VAULT").expect("set LODESTAR_VAULT");
        let cs = list_companies(path).unwrap();
        println!("parsed {} companies", cs.len());
        assert_eq!(cs.len(), 179, "all real company notes should parse");
        assert!(cs.iter().any(|c| c.slug == "stripe" && c.name == "Stripe"));
        let names: Vec<_> = cs.iter().map(|c| c.name.to_lowercase()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]), "should be name-sorted");
    }

    #[test]
    #[ignore = "reads a real note; run with LODESTAR_VAULT set"]
    fn set_field_preserves_real_note() {
        let vault = std::env::var("LODESTAR_VAULT").expect("set LODESTAR_VAULT");
        let text =
            std::fs::read_to_string(format!("{vault}/companies/stripe.md")).unwrap();
        let before = parse_company("stripe", &text, d("2026-06-15")).unwrap();
        let out = set_frontmatter_field(&text, "status", "paused").unwrap();
        let after = parse_company("stripe", &out, d("2026-06-15")).unwrap();
        assert_eq!(after.status.as_deref(), Some("paused"));
        assert_eq!(after.name, before.name);
        assert_eq!(after.domain, before.domain);
        assert_eq!(after.careers_url, before.careers_url);
        assert_eq!(after.notes, before.notes); // body untouched
        assert_eq!(out, text.replacen("status: active", "status: paused", 1));
    }
}
