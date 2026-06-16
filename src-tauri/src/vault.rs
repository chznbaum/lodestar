use chrono::{Local, NaiveDate};
use serde::Serialize;
use std::path::Path;

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
}

/// Split a note into (frontmatter_yaml, body). Returns ("", whole) if no frontmatter.
fn split_frontmatter(text: &str) -> (&str, &str) {
    let after = match text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n")) {
        Some(rest) => rest,
        None => return ("", text),
    };
    match after.split_once("\n---") {
        Some((fm, body)) => (fm, body.trim_start_matches(['\r', '\n']).trim_start()),
        None => ("", text),
    }
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
        due_for_check,
        screening,
        notes: body.trim().to_string(),
    })
}

#[tauri::command]
pub fn list_companies(vault_path: String) -> Result<Vec<Company>, String> {
    let today = Local::now().date_naive();
    let dir = Path::new(&vault_path).join("companies");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("read {dir:?}: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if slug.starts_with('_') {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match parse_company(&slug, &text, today) {
            Ok(c) => out.push(c),
            Err(e) => eprintln!("skip {slug}: {e}"),
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Replace `key: ...` inside the frontmatter, preserving everything else byte-for-byte.
/// Inserts the key just before the closing `---` if absent. Errors if there's no frontmatter.
pub fn set_frontmatter_field(text: &str, key: &str, value: &str) -> Result<String, String> {
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
pub fn set_body(text: &str, body: &str) -> String {
    match text.find("\n---") {
        Some(i) => {
            let after = &text[i + 1..]; // "---...body"
            let close_end = after.find('\n').map(|n| i + 1 + n + 1).unwrap_or(text.len());
            format!("{}\n{}\n", text[..close_end].trim_end(), body)
        }
        None => format!("{}\n", body),
    }
}

fn company_path(vault_path: &str, slug: &str) -> std::path::PathBuf {
    Path::new(vault_path)
        .join("companies")
        .join(format!("{slug}.md"))
}

#[tauri::command]
pub fn update_company_field(
    vault_path: String,
    slug: String,
    key: String,
    value: String,
) -> Result<Company, String> {
    let p = company_path(&vault_path, &slug);
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let updated = set_frontmatter_field(&text, &key, &value)?;
    std::fs::write(&p, &updated).map_err(|e| e.to_string())?;
    parse_company(&slug, &updated, Local::now().date_naive())
}

#[tauri::command]
pub fn set_company_notes(vault_path: String, slug: String, body: String) -> Result<Company, String> {
    let p = company_path(&vault_path, &slug);
    let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let updated = set_body(&text, &body);
    std::fs::write(&p, &updated).map_err(|e| e.to_string())?;
    parse_company(&slug, &updated, Local::now().date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

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
    #[ignore = "reads a real note; run with LODESTAR_VAULT set"]
    fn set_field_preserves_real_note() {
        let vault = std::env::var("LODESTAR_VAULT").expect("set LODESTAR_VAULT");
        let text = std::fs::read_to_string(format!("{vault}/companies/stripe.md")).unwrap();
        let before = parse_company("stripe", &text, d("2026-06-15")).unwrap();
        let out = set_frontmatter_field(&text, "status", "paused").unwrap();
        let after = parse_company("stripe", &out, d("2026-06-15")).unwrap();
        assert_eq!(after.status.as_deref(), Some("paused"));
        assert_eq!(after.name, before.name);
        assert_eq!(after.domain, before.domain);
        assert_eq!(after.careers_url, before.careers_url);
        assert_eq!(after.notes, before.notes); // body untouched
        // byte-identical except the one status line
        assert_eq!(out, text.replacen("status: active", "status: paused", 1));
    }

    #[test]
    fn set_body_replaces_body_keeps_frontmatter() {
        let t = "---\nid: stripe\nstatus: active\n---\n\nold body\n";
        let out = set_body(t, "new body");
        assert!(out.starts_with("---\nid: stripe\nstatus: active\n---\n"));
        assert!(out.trim_end().ends_with("new body"));
        assert!(!out.contains("old body"));
    }

    #[test]
    #[ignore = "needs the real vault; run with LODESTAR_VAULT=<path> cargo test -- --ignored --nocapture"]
    fn smoke_parses_real_vault() {
        let path = std::env::var("LODESTAR_VAULT").expect("set LODESTAR_VAULT");
        let cs = list_companies(path).unwrap();
        println!("parsed {} companies", cs.len());
        assert_eq!(cs.len(), 179, "all real company notes should parse");
        assert!(cs.iter().any(|c| c.slug == "stripe" && c.name == "Stripe"));
        // sorted by name, case-insensitive
        let names: Vec<_> = cs.iter().map(|c| c.name.to_lowercase()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]), "should be name-sorted");
    }

    #[test]
    fn ignores_unknown_frontmatter_keys() {
        // real notes carry extra keys (business_model, domain_raw, source, …)
        let text = "---\nid: x\nname: \"X\"\nstatus: paused\nbusiness_model: [b2b]\ndomain_raw: \"whatever\"\nsource: \"claude_research_batch_1\"\n---\n\nbody\n";
        let c = parse_company("x", text, d("2026-06-15")).unwrap();
        assert_eq!(c.name, "X");
        assert_eq!(c.status.as_deref(), Some("paused"));
        assert!(!c.due_for_check); // not active
    }
}
