//! The `Domain` controlled-vocabulary entity: parsing, the list command, and the
//! screening map. Mirrors `company.rs`; uses `crate::note` for frontmatter I/O.

use crate::note::{self, split_frontmatter};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, PartialEq)]
pub struct Domain {
    pub slug: String,
    pub name: String,
    pub aliases: Vec<String>,
    /// "dealbreaker" | "caution" | None
    pub screening: Option<String>,
}

#[derive(serde::Deserialize)]
struct Front {
    name: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    screening: Option<String>,
}

pub fn parse_domain(slug: &str, text: &str) -> Result<Domain, String> {
    let (fm, _body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    Ok(Domain {
        slug: slug.to_string(),
        name: f.name.unwrap_or_else(|| slug.to_string()),
        aliases: f.aliases,
        screening: f.screening,
    })
}

/// Read every domain note under `<vault>/domains/`. Errors if the dir can't be read.
fn read_domains(vault_path: &str) -> Result<Vec<Domain>, String> {
    let dir = Path::new(vault_path).join("domains");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("read {dir:?}: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let Some(slug) = note::note_slug(file_name) else {
            continue;
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        match parse_domain(&slug, &text) {
            Ok(d) => out.push(d),
            Err(e) => eprintln!("skip domain {slug}: {e}"),
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn list_domains(vault_path: String) -> Result<Vec<Domain>, String> {
    let mut out = read_domains(&vault_path)?;
    out.sort_by_key(|a| a.name.to_lowercase());
    Ok(out)
}

/// slug -> "dealbreaker"|"caution", built from the domain notes' `screening` field.
/// Swallows a missing/unreadable `domains/` dir (returns empty) so company loading never
/// fails just because domains aren't present — screening simply degrades to None.
pub fn screening_map(vault_path: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(domains) = read_domains(vault_path) {
        for d in domains {
            if let Some(s) = d.screening {
                map.insert(d.slug, s);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nid: financial_services\nname: \"Financial Services\"\naliases: [\"Fintech\", \"Banking\"]\n---\n";
    const SCREENED: &str = "---\nid: crypto_web3\nname: \"Crypto / Web3\"\naliases: [\"Crypto\", \"Web3\"]\nscreening: dealbreaker\n---\n";
    const NO_NAME: &str = "---\nid: iot\naliases: [\"Internet of Things\"]\n---\n";

    #[test]
    fn parses_name_aliases_and_no_screening() {
        let d = parse_domain("financial_services", SAMPLE).unwrap();
        assert_eq!(d.slug, "financial_services");
        assert_eq!(d.name, "Financial Services");
        assert_eq!(
            d.aliases,
            vec!["Fintech".to_string(), "Banking".to_string()]
        );
        assert_eq!(d.screening, None);
    }

    #[test]
    fn parses_screening_field() {
        let d = parse_domain("crypto_web3", SCREENED).unwrap();
        assert_eq!(d.screening.as_deref(), Some("dealbreaker"));
    }

    #[test]
    fn name_falls_back_to_slug_when_absent() {
        let d = parse_domain("iot", NO_NAME).unwrap();
        assert_eq!(d.name, "iot");
        assert_eq!(d.aliases, vec!["Internet of Things".to_string()]);
    }

    #[test]
    fn list_sorts_by_name_skips_underscored_and_maps_screening() {
        let dir = std::env::temp_dir().join(format!("lodestar-dom-test-{}", std::process::id()));
        let domains = dir.join("domains");
        std::fs::create_dir_all(&domains).unwrap();
        std::fs::write(domains.join("crypto_web3.md"), SCREENED).unwrap();
        std::fs::write(domains.join("financial_services.md"), SAMPLE).unwrap();
        std::fs::write(domains.join("iot.md"), NO_NAME).unwrap();
        std::fs::write(domains.join("_template.md"), SAMPLE).unwrap(); // must be skipped
        let vault = dir.to_str().unwrap().to_string();

        let list = list_domains(vault.clone()).unwrap();
        let names: Vec<_> = list.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["Crypto / Web3", "Financial Services", "iot"]);

        let map = screening_map(&vault);
        assert_eq!(
            map.get("crypto_web3").map(String::as_str),
            Some("dealbreaker")
        );
        assert_eq!(map.get("financial_services"), None);
        assert_eq!(map.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn screening_map_missing_dir_is_empty() {
        assert!(screening_map("/no/such/vault/path").is_empty());
    }
}
