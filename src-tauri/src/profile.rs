//! Reads the user's targeting from `profile/target_criteria.md`. `match_titles` is the
//! recall-oriented expanded alias list the discovery pre-filter matches against; `remote_only`
//! is derived from the note's `location_requirement` field.
// Consumed by the discovery chain (Task 5); suppress dead-code until wired.
#![allow(dead_code)]

use crate::note::split_frontmatter;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct TargetCriteria {
    pub match_titles: Vec<String>,
    /// True when the note's `location_requirement` is `remote_only`.
    pub remote_only: bool,
}

#[derive(Deserialize)]
struct Front {
    #[serde(default)]
    match_titles: Vec<String>,
    location_requirement: Option<String>,
}

pub fn parse_target_criteria(text: &str) -> Result<TargetCriteria, String> {
    let (fm, _body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| e.to_string())?;
    Ok(TargetCriteria {
        match_titles: f.match_titles,
        remote_only: f.location_requirement.as_deref() == Some("remote_only"),
    })
}

pub fn read_target_criteria(vault_path: &str) -> Result<TargetCriteria, String> {
    let p = Path::new(vault_path).join("profile").join("target_criteria.md");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
    parse_target_criteria(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "---\ntype: target_criteria\nlocation_requirement: remote_only\nmatch_titles:\n  - founding engineer\n  - ai engineer\n---\n\nbody\n";

    #[test]
    fn parses_match_titles_and_remote_flag() {
        let c = parse_target_criteria(FIXTURE).unwrap();
        assert_eq!(
            c.match_titles,
            vec!["founding engineer".to_string(), "ai engineer".to_string()]
        );
        assert!(c.remote_only);
    }

    #[test]
    fn missing_fields_default_safely() {
        let c = parse_target_criteria("---\ntype: target_criteria\n---\n").unwrap();
        assert!(c.match_titles.is_empty());
        assert!(!c.remote_only); // no location_requirement -> not remote-only
    }

    #[test]
    fn non_remote_location_requirement_is_not_remote_only() {
        let c = parse_target_criteria("---\nlocation_requirement: hybrid\n---\n").unwrap();
        assert!(!c.remote_only);
    }
}
