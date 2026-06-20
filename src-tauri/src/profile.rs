//! Reads the user's targeting from `profile/target_criteria.md`. `match_titles` is the
//! recall-oriented expanded alias list the discovery pre-filter matches against; `remote_only`
//! is derived from the note's `location_requirement` field.
// Consumed by the discovery chain (Task 5); suppress dead-code until wired.
#![allow(dead_code)]

use crate::note::split_frontmatter;
use serde::Deserialize;
use std::path::Path;

/// Per-dimension weights used for scoring jobs against the user's criteria.
/// All four weights must sum to 1.0. `Default` returns the recommended baseline.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FitWeights {
    pub seniority: f64,
    pub skills: f64,
    pub comp: f64,
    pub domain: f64,
}

impl Default for FitWeights {
    fn default() -> Self {
        Self {
            seniority: 0.3,
            skills: 0.35,
            comp: 0.25,
            domain: 0.10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetCriteria {
    pub match_titles: Vec<String>,
    /// True when the note's `location_requirement` is `remote_only`.
    pub remote_only: bool,
    // --- fit configuration ---
    pub target_levels: Vec<String>,
    pub comp_floor: Option<i64>,
    pub comp_target: Option<i64>,
    pub comp_currency: Option<String>,
    pub employment_types: Vec<String>,
    pub open_to_relocation: bool,
    pub work_authorization: Vec<String>,
    pub requires_sponsorship: bool,
    pub preferred_domains: Vec<String>,
    pub avoid_domains: Vec<String>,
    pub fit_weights: FitWeights,
}

#[derive(Deserialize)]
struct Front {
    #[serde(default)]
    match_titles: Vec<String>,
    location_requirement: Option<String>,
    // --- fit configuration ---
    #[serde(default)]
    target_levels: Vec<String>,
    #[serde(default)]
    comp_floor: Option<i64>,
    #[serde(default)]
    comp_target: Option<i64>,
    #[serde(default)]
    comp_currency: Option<String>,
    #[serde(default)]
    employment_types: Vec<String>,
    #[serde(default)]
    open_to_relocation: bool,
    #[serde(default)]
    work_authorization: Vec<String>,
    #[serde(default)]
    requires_sponsorship: bool,
    #[serde(default)]
    preferred_domains: Vec<String>,
    #[serde(default)]
    avoid_domains: Vec<String>,
    #[serde(default)]
    fit_weights: FitWeights,
}

pub fn parse_target_criteria(text: &str) -> Result<TargetCriteria, String> {
    let (fm, _body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| e.to_string())?;
    Ok(TargetCriteria {
        match_titles: f.match_titles,
        remote_only: f.location_requirement.as_deref() == Some("remote_only"),
        target_levels: f.target_levels,
        comp_floor: f.comp_floor,
        comp_target: f.comp_target,
        comp_currency: f.comp_currency,
        employment_types: f.employment_types,
        open_to_relocation: f.open_to_relocation,
        work_authorization: f.work_authorization,
        requires_sponsorship: f.requires_sponsorship,
        preferred_domains: f.preferred_domains,
        avoid_domains: f.avoid_domains,
        fit_weights: f.fit_weights,
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

    const FULL: &str = "---\ntype: target_criteria\nlocation_requirement: remote_only\nmatch_titles:\n  - founding engineer\ntarget_levels: [senior, dept-head]\ncomp_floor: 180000\ncomp_target: 220000\ncomp_currency: USD\nemployment_types: [full_time, fractional]\nopen_to_relocation: false\nwork_authorization: [US]\nrequires_sponsorship: false\npreferred_domains: [dev_tools]\navoid_domains: [gambling]\nfit_weights: { seniority: 0.25, skills: 0.4, comp: 0.25, domain: 0.1 }\n---\n";

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

    #[test]
    fn parses_full_fit_config() {
        let c = parse_target_criteria(FULL).unwrap();
        assert_eq!(c.target_levels, vec!["senior", "dept-head"]);
        assert_eq!(c.comp_floor, Some(180000));
        assert_eq!(c.comp_target, Some(220000));
        assert_eq!(c.employment_types, vec!["full_time", "fractional"]);
        assert!(!c.open_to_relocation);
        assert_eq!(c.work_authorization, vec!["US"]);
        assert!(!c.requires_sponsorship);
        assert_eq!(c.avoid_domains, vec!["gambling"]);
        assert!((c.fit_weights.skills - 0.4).abs() < 1e-9);
    }

    #[test]
    fn fit_weights_default_when_absent_sum_to_one() {
        let c = parse_target_criteria("---\ntype: target_criteria\n---\n").unwrap();
        let w = &c.fit_weights;
        assert!((w.seniority + w.skills + w.comp + w.domain - 1.0).abs() < 1e-9);
        assert!(c.target_levels.is_empty()); // safe defaults
    }
}
