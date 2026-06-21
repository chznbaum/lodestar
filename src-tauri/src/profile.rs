//! Reads the user's targeting from `profile/target_criteria.md`. `match_titles` is the
//! recall-oriented expanded alias list the discovery pre-filter matches against;
//! `work_arrangements` lists the candidate's acceptable arrangements (e.g. `["remote"]`).
// Consumed by the discovery chain (Task 5); suppress dead-code until wired.
#![allow(dead_code)]

use crate::note::split_frontmatter;
use serde::Deserialize;
use std::path::Path;

/// Per-dimension weights used for scoring jobs against the user's criteria.
/// All five weights must sum to 1.0. `Default` returns the recommended baseline.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FitWeights {
    pub seniority: f64,
    pub skills: f64,
    pub comp: f64,
    pub arrangement: f64,
    pub domain: f64,
}

impl Default for FitWeights {
    fn default() -> Self {
        Self {
            seniority: 0.20,
            skills: 0.25,
            comp: 0.30,
            arrangement: 0.15,
            domain: 0.10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetCriteria {
    pub match_titles: Vec<String>,
    pub target_titles: Vec<String>,
    pub work_arrangements: Vec<String>,
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
    // --- location ---
    pub current_location: Option<String>,
    pub preferred_locations: Vec<String>,
}

#[derive(Deserialize)]
struct Front {
    #[serde(default)]
    match_titles: Vec<String>,
    #[serde(default)]
    target_titles: Vec<String>,
    #[serde(default)]
    work_arrangements: Vec<String>,
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
    // --- location ---
    #[serde(default)]
    current_location: Option<String>,
    #[serde(default)]
    preferred_locations: Vec<String>,
}

pub fn parse_target_criteria(text: &str) -> Result<TargetCriteria, String> {
    let (fm, _body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| e.to_string())?;
    Ok(TargetCriteria {
        match_titles: f.match_titles,
        target_titles: f.target_titles,
        work_arrangements: f.work_arrangements,
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
        current_location: f.current_location,
        preferred_locations: f.preferred_locations,
    })
}

pub fn read_target_criteria(vault_path: &str) -> Result<TargetCriteria, String> {
    let p = Path::new(vault_path).join("profile").join("target_criteria.md");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
    parse_target_criteria(&text)
}

/// `(slug, headline)` for every accomplishment note in `profile/accomplishments/`,
/// stable-sorted by slug. The headline is the note's `headline:` frontmatter (empty when
/// absent — the note stays citable by slug). Used as the citable evidence list for the
/// qualitative `alignment` step.
pub fn list_accomplishments(vault_path: &str) -> Result<Vec<(String, String)>, String> {
    let dir = Path::new(vault_path).join("profile").join("accomplishments");
    let mut out = crate::note::read_notes_in(&dir, |slug, text| {
        #[derive(Deserialize)]
        struct AccFront {
            headline: Option<String>,
        }
        let (fm, _body) = split_frontmatter(text);
        let f: AccFront = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
        Ok((slug.to_string(), f.headline.unwrap_or_default().trim().to_string()))
    })?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The positioning narrative body (everything after the frontmatter) from
/// `profile/positioning.md`, trimmed. The `type`/`status` frontmatter is dropped — only the
/// narrative is useful as alignment context.
pub fn read_positioning(vault_path: &str) -> Result<String, String> {
    let p = Path::new(vault_path).join("profile").join("positioning.md");
    let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
    let (_fm, body) = split_frontmatter(&text);
    Ok(body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "---\ntype: target_criteria\nwork_arrangements: [remote]\nmatch_titles:\n  - founding engineer\n  - ai engineer\n---\n\nbody\n";

    const FULL: &str = "---\ntype: target_criteria\nwork_arrangements: [remote]\ntarget_titles: [\"Founding Engineer\", \"Senior Software Engineer\"]\nmatch_titles:\n  - founding engineer\ntarget_levels: [senior, dept-head]\ncomp_floor: 180000\ncomp_target: 220000\ncomp_currency: USD\nemployment_types: [full_time, fractional]\nopen_to_relocation: false\nwork_authorization: [US]\nrequires_sponsorship: false\npreferred_domains: [dev_tools]\navoid_domains: [gambling]\nfit_weights: { seniority: 0.25, skills: 0.4, comp: 0.25, arrangement: 0.0, domain: 0.1 }\n---\n";

    #[test]
    fn parses_match_titles_and_work_arrangements() {
        let c = parse_target_criteria(FIXTURE).unwrap();
        assert_eq!(
            c.match_titles,
            vec!["founding engineer".to_string(), "ai engineer".to_string()]
        );
        assert_eq!(c.work_arrangements, vec!["remote".to_string()]);
    }

    #[test]
    fn missing_fields_default_safely() {
        let c = parse_target_criteria("---\ntype: target_criteria\n---\n").unwrap();
        assert!(c.match_titles.is_empty());
        assert!(c.work_arrangements.is_empty()); // absent -> empty vec
        assert!(c.target_titles.is_empty());
    }

    #[test]
    fn non_remote_work_arrangements_parses() {
        let c = parse_target_criteria("---\nwork_arrangements: [hybrid, onsite]\n---\n").unwrap();
        assert_eq!(c.work_arrangements, vec!["hybrid".to_string(), "onsite".to_string()]);
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
        assert_eq!(
            c.target_titles,
            vec!["Founding Engineer".to_string(), "Senior Software Engineer".to_string()]
        );
    }

    #[test]
    fn parses_location_fields() {
        let text = "---\ntype: target_criteria\ncurrent_location: norfolk-va\npreferred_locations: [richmond-va, washington-arlington-alexandria-dc-va-md-wv]\n---\n";
        let c = parse_target_criteria(text).unwrap();
        assert_eq!(c.current_location.as_deref(), Some("norfolk-va"));
        assert_eq!(
            c.preferred_locations,
            vec!["richmond-va", "washington-arlington-alexandria-dc-va-md-wv"]
        );
    }

    #[test]
    fn location_fields_default_when_absent() {
        let c = parse_target_criteria("---\ntype: target_criteria\n---\n").unwrap();
        assert_eq!(c.current_location, None);
        assert!(c.preferred_locations.is_empty());
    }

    #[test]
    fn fit_weights_default_when_absent_sum_to_one() {
        let c = parse_target_criteria("---\ntype: target_criteria\n---\n").unwrap();
        let w = &c.fit_weights;
        assert!(
            (w.seniority + w.skills + w.comp + w.arrangement + w.domain - 1.0).abs() < 1e-9,
            "weights sum to {}, not 1.0",
            w.seniority + w.skills + w.comp + w.arrangement + w.domain
        );
        assert!(c.target_levels.is_empty()); // safe defaults
    }

    // ── list_accomplishments ───────────────────────────────────────────────────

    #[test]
    fn list_accomplishments_returns_sorted_slug_headline_pairs() {
        let dir = std::env::temp_dir().join(format!("lodestar-acc-{}", std::process::id()));
        let acc = dir.join("profile").join("accomplishments");
        std::fs::create_dir_all(&acc).unwrap();
        std::fs::write(
            acc.join("zeta-win.md"),
            "---\nid: zeta-win\nheadline: \"Shipped Zeta end to end.\"\ndemonstrates: [\"[[rust]]\"]\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(
            acc.join("alpha-win.md"),
            "---\nid: alpha-win\nheadline: \"Cut infra spend 30%.\"\n---\nbody\n",
        )
        .unwrap();
        let got = list_accomplishments(dir.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            got,
            vec![
                ("alpha-win".to_string(), "Cut infra spend 30%.".to_string()),
                ("zeta-win".to_string(), "Shipped Zeta end to end.".to_string()),
            ]
        );
    }

    #[test]
    fn list_accomplishments_missing_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!("lodestar-acc-none-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert!(list_accomplishments(dir.to_str().unwrap()).unwrap().is_empty());
    }

    // ── read_positioning ────────────────────────────────────────────────────────

    #[test]
    fn read_positioning_returns_body_without_frontmatter() {
        let dir = std::env::temp_dir().join(format!("lodestar-pos-{}", std::process::id()));
        let prof = dir.join("profile");
        std::fs::create_dir_all(&prof).unwrap();
        std::fs::write(
            prof.join("positioning.md"),
            "---\ntype: positioning\nstatus: draft\n---\n## Primary narrative\nI'm a founding engineer.\n",
        )
        .unwrap();
        let body = read_positioning(dir.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(body.contains("## Primary narrative"));
        assert!(body.contains("I'm a founding engineer."));
        assert!(!body.contains("type: positioning"), "frontmatter leaked: {body}");
    }

    #[test]
    #[ignore]
    fn smoke_parses_real_vault_target_criteria() {
        let vault = std::env::var("LODESTAR_VAULT")
            .expect("LODESTAR_VAULT must be set to run this smoke test");
        let c = read_target_criteria(&vault)
            .expect("parse_target_criteria should succeed on the real vault note");
        assert!(
            !c.target_levels.is_empty(),
            "target_levels must be non-empty; got empty"
        );
        assert!(
            c.comp_floor.is_some(),
            "comp_floor must be Some; got None"
        );
        assert!(
            !c.work_arrangements.is_empty(),
            "work_arrangements must be non-empty; got empty"
        );
        assert!(
            !c.work_authorization.is_empty(),
            "work_authorization must be non-empty; got empty"
        );
        println!("target_levels: {:?}", c.target_levels);
        println!("comp_floor: {:?}", c.comp_floor);
        println!("work_arrangements: {:?}", c.work_arrangements);
        println!("work_authorization: {:?}", c.work_authorization);
        println!("target_titles: {:?}", c.target_titles);
        println!("fit_weights: seniority={} skills={} comp={} arrangement={} domain={}",
            c.fit_weights.seniority, c.fit_weights.skills, c.fit_weights.comp,
            c.fit_weights.arrangement, c.fit_weights.domain);
    }
}
