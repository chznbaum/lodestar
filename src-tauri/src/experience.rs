//! Experience entity: parses the user's career history from `profile/experience/*.md`
//! and computes total years of experience from date-spans.
#![allow(dead_code)]

use crate::note::{self, split_frontmatter};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::path::Path;

pub struct Experience {
    pub slug: String,
    pub company: String,
    pub role_title: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_current: bool,
    pub location: Option<String>,
    pub remote: Option<bool>,
    pub competencies: Vec<String>,
    pub tagline: Option<String>,
    /// The note body (everything after the frontmatter), trimmed. Carries the
    /// `## Summary` / `## Progression` prose used as qualitative-alignment context.
    pub body: String,
}

#[derive(Deserialize)]
struct Front {
    company: Option<String>,
    role_title: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    is_current: Option<bool>,
    location: Option<String>,
    remote: Option<bool>,
    #[serde(default)]
    competencies: Vec<String>,
    tagline: Option<String>,
}

/// Treat a blank string (None or whitespace-only) as absent.
fn nonempty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty())
}

fn parse_experience(slug: &str, text: &str) -> Result<Experience, String> {
    let (fm, body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;

    let start_date = nonempty(f.start_date);
    let end_date = nonempty(f.end_date);

    // Prefer explicit `is_current`; fall back: current iff `end_date` is absent/blank.
    let is_current = f
        .is_current
        .unwrap_or_else(|| end_date.as_deref().is_none_or(|s| s.trim().is_empty()));

    let competencies = f.competencies.iter().map(|c| note::strip_wikilink(c)).collect();

    Ok(Experience {
        slug: slug.to_string(),
        company: f.company.unwrap_or_default(),
        role_title: f.role_title.unwrap_or_default(),
        start_date,
        end_date,
        is_current,
        location: nonempty(f.location),
        remote: f.remote,
        competencies,
        tagline: nonempty(f.tagline),
        body: body.trim().to_string(),
    })
}

pub fn list_experiences(vault_path: &str) -> Result<Vec<Experience>, String> {
    note::read_notes_in(
        &Path::new(vault_path).join("profile").join("experience"),
        parse_experience,
    )
}

/// Parse a `YYYY-MM` date string to the 1st of that month. Returns `None` for any
/// unparseable or missing input (recall-safe — never panics).
fn parse_ym(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").ok()
}

/// Total career span in whole months, computed as earliest-start → latest-end across
/// all experiences. Gaps and overlaps are ignored (it's the envelope, not summed segments).
///
/// - Roles whose `start_date` cannot be parsed are excluded from the earliest-start
///   calculation (they contribute nothing).
/// - `is_current` roles contribute `today` as their end; all others contribute their
///   parsed `end_date` (skipped if unparseable).
/// - If no valid start dates exist → `0`.
/// - If `latest <= earliest` → `0`.
pub fn total_months_experience(exps: &[Experience], today: NaiveDate) -> i64 {
    let starts: Vec<NaiveDate> = exps
        .iter()
        .filter_map(|e| e.start_date.as_deref().and_then(parse_ym))
        .collect();

    let Some(earliest) = starts.iter().copied().min() else {
        return 0;
    };

    let ends: Vec<NaiveDate> = exps
        .iter()
        .filter_map(|e| {
            if e.is_current {
                Some(today)
            } else {
                e.end_date.as_deref().and_then(parse_ym)
            }
        })
        .collect();

    let latest = ends.iter().copied().max().unwrap_or(today);

    if latest <= earliest {
        return 0;
    }

    // Count whole months: each full calendar month between earliest and latest.
    let years = latest.year() as i64 - earliest.year() as i64;
    let months = latest.month() as i64 - earliest.month() as i64;
    years * 12 + months
}

/// Total career span in fractional years, computed as earliest-start → latest-end across
/// all experiences. Gaps and overlaps are ignored (it's the envelope, not summed segments).
///
/// - Roles whose `start_date` cannot be parsed are excluded from the earliest-start
///   calculation (they contribute nothing).
/// - `is_current` roles contribute `today` as their end; all others contribute their
///   parsed `end_date` (skipped if unparseable).
/// - If no valid start dates exist → `0.0`.
/// - If `latest <= earliest` → `0.0`.
pub fn total_years_experience(exps: &[Experience], today: NaiveDate) -> f64 {
    let starts: Vec<NaiveDate> = exps
        .iter()
        .filter_map(|e| e.start_date.as_deref().and_then(parse_ym))
        .collect();

    let Some(earliest) = starts.iter().copied().min() else {
        return 0.0;
    };

    let ends: Vec<NaiveDate> = exps
        .iter()
        .filter_map(|e| {
            if e.is_current {
                Some(today)
            } else {
                e.end_date.as_deref().and_then(parse_ym)
            }
        })
        .collect();

    let latest = ends.iter().copied().max().unwrap_or(today);

    if latest <= earliest {
        return 0.0;
    }

    (latest - earliest).num_days() as f64 / 365.25
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exp(
        slug: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
        is_current: bool,
        competencies: Vec<&str>,
    ) -> Experience {
        Experience {
            slug: slug.to_string(),
            company: "Test Co".to_string(),
            role_title: "Engineer".to_string(),
            start_date: start_date.map(String::from),
            end_date: end_date.map(String::from),
            is_current,
            location: None,
            remote: None,
            competencies: competencies.iter().map(|s| s.to_string()).collect(),
            tagline: None,
            body: String::new(),
        }
    }

    // ── parse_experience body capture ──────────────────────────────────────────

    #[test]
    fn parse_experience_captures_note_body() {
        let text = "---\ncompany: MAXX Potential\nrole_title: Site Lead\nstart_date: 2018-01\nend_date: 2022-01\n---\n## Summary\nLed a Norfolk office of 8 concurrent teams.\n\n## Progression\nApprentice → Site Lead.\n";
        let exp = parse_experience("maxx-site-lead", text).unwrap();
        assert_eq!(exp.role_title, "Site Lead");
        assert!(exp.body.contains("## Summary"), "body missing Summary: {:?}", exp.body);
        assert!(exp.body.contains("Led a Norfolk office of 8 concurrent teams."));
        assert!(exp.body.contains("## Progression"));
    }

    // ── total_years_experience ────────────────────────────────────────────────

    #[test]
    fn current_role_from_2018_01_to_2025_01() {
        let exps = vec![make_exp("role-a", Some("2018-01"), None, true, vec![])];
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let yoe = total_years_experience(&exps, today);
        // 2018-01-01 to 2025-01-01 = exactly 7 years
        assert!(
            (yoe - 7.0).abs() < 0.05,
            "expected ~7.0 yoe, got {yoe:.4}"
        );
    }

    #[test]
    fn earliest_start_to_latest_end_across_several_roles() {
        // role-a: 2010-03 to 2012-06 (past)
        // role-b: 2018-01 to current (today = 2026-06-20)
        let exps = vec![
            make_exp("role-a", Some("2010-03"), Some("2012-06"), false, vec![]),
            make_exp("role-b", Some("2018-01"), None, true, vec![]),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        // earliest = 2010-03-01, latest = 2026-06-20
        // days = (2026-06-20) - (2010-03-01) = 5955 days → ~16.3 years
        let yoe = total_years_experience(&exps, today);
        // span is from 2010-03-01 to 2026-06-20
        let expected_days =
            (today - NaiveDate::from_ymd_opt(2010, 3, 1).unwrap()).num_days() as f64;
        let expected = expected_days / 365.25;
        assert!(
            (yoe - expected).abs() < 0.01,
            "expected ~{expected:.4} yoe, got {yoe:.4}"
        );
    }

    #[test]
    fn role_with_no_dates_skipped_does_not_panic() {
        let exps = vec![
            make_exp("no-dates", None, None, false, vec![]),
            make_exp("role-b", Some("2018-01"), None, true, vec![]),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        // no-dates contributes nothing; result is driven by role-b alone
        let yoe = total_years_experience(&exps, today);
        assert!(yoe > 0.0, "expected positive yoe, got {yoe}");
        // And the no-dates role itself, given as the only entry, should yield 0.0
    }

    #[test]
    fn only_undated_roles_returns_zero() {
        let exps = vec![
            make_exp("a", None, None, false, vec![]),
            make_exp("b", None, None, true, vec![]),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        assert_eq!(total_years_experience(&exps, today), 0.0);
    }

    #[test]
    fn empty_slice_returns_zero() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        assert_eq!(total_years_experience(&[], today), 0.0);
    }

    // ── total_months_experience ───────────────────────────────────────────────

    #[test]
    fn months_current_role_from_2018_01_to_2025_01_is_84() {
        let exps = vec![make_exp("role-a", Some("2018-01"), None, true, vec![])];
        let today = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        // 2018-01 to 2025-01 = 7 years = 84 months
        assert_eq!(total_months_experience(&exps, today), 84);
    }

    #[test]
    fn months_exact_count_across_roles() {
        // role-a: 2010-03 to 2012-06 (past)
        // role-b: 2018-01 to 2026-06 (today = 2026-06-20, same month)
        let exps = vec![
            make_exp("role-a", Some("2010-03"), Some("2012-06"), false, vec![]),
            make_exp("role-b", Some("2018-01"), None, true, vec![]),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        // earliest = 2010-03, latest = 2026-06 → (2026-2010)*12 + (6-3) = 16*12 + 3 = 195
        assert_eq!(total_months_experience(&exps, today), 195);
    }

    #[test]
    fn months_empty_slice_returns_zero() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        assert_eq!(total_months_experience(&[], today), 0);
    }

    #[test]
    fn months_only_undated_roles_returns_zero() {
        let exps = vec![
            make_exp("a", None, None, false, vec![]),
            make_exp("b", None, None, true, vec![]),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        assert_eq!(total_months_experience(&exps, today), 0);
    }

    // ── parse_experience ──────────────────────────────────────────────────────

    #[test]
    fn parse_strips_wikilinks_from_competencies() {
        let text = "---\ncompany: Acme\nrole_title: Dev\nstart_date: 2020-01\ncompetencies: [\"[[a]]\", \"[[b-c]]\"]\n---\nbody\n";
        let exp = parse_experience("acme-dev", text).unwrap();
        assert_eq!(exp.competencies, vec!["a", "b-c"]);
    }

    #[test]
    fn parse_is_current_derived_true_when_end_date_blank() {
        // No `is_current` field; `end_date` is absent → derived current
        let text = "---\ncompany: Acme\nrole_title: Dev\nstart_date: 2020-01\n---\nbody\n";
        let exp = parse_experience("acme-dev", text).unwrap();
        assert!(exp.is_current);
    }

    #[test]
    fn parse_is_current_derived_false_when_end_date_present() {
        // No `is_current` field; `end_date` present → derived NOT current
        let text = "---\ncompany: Acme\nrole_title: Dev\nstart_date: 2020-01\nend_date: 2022-06\n---\nbody\n";
        let exp = parse_experience("acme-dev", text).unwrap();
        assert!(!exp.is_current);
        assert_eq!(exp.end_date.as_deref(), Some("2022-06"));
    }

    #[test]
    fn parse_explicit_is_current_overrides_end_date_derivation() {
        // Explicit `is_current: false` with no end_date → still false (explicit wins)
        let text = "---\ncompany: Acme\nrole_title: Dev\nstart_date: 2020-01\nis_current: false\n---\nbody\n";
        let exp = parse_experience("acme-dev", text).unwrap();
        assert!(!exp.is_current);
        // Explicit `is_current: true` with an end_date → still true (explicit wins)
        let text2 = "---\ncompany: Acme\nrole_title: Dev\nstart_date: 2020-01\nend_date: 2022-06\nis_current: true\n---\nbody\n";
        let exp2 = parse_experience("acme-dev", text2).unwrap();
        assert!(exp2.is_current);
    }

    #[test]
    fn parse_blank_dates_treated_as_none() {
        // YAML scalar blank for start_date / end_date → both None after parsing
        let text = "---\ncompany: Walmart\nrole_title: Greeter\nstart_date:\nend_date:\ncompetencies: []\n---\nbody\n";
        let exp = parse_experience("walmart", text).unwrap();
        assert!(exp.start_date.is_none());
        assert!(exp.end_date.is_none());
        // No explicit `is_current` and end_date is blank → derived current
        assert!(exp.is_current);
    }

    // ── vault smoke test ──────────────────────────────────────────────────────

    #[test]
    #[ignore = "reads the real vault; run with LODESTAR_VAULT=<path> cargo test -- --ignored --nocapture"]
    fn smoke_parses_real_vault_experiences() {
        let path = std::env::var("LODESTAR_VAULT").expect("set LODESTAR_VAULT");
        let exps = list_experiences(&path).unwrap();
        println!("parsed {} experiences", exps.len());
        assert_eq!(exps.len(), 12, "expected all 12 experience notes to parse");

        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let yoe = total_years_experience(&exps, today);
        println!("total_years_experience = {yoe:.2}");
        assert!(yoe > 10.0, "real career spans ~2009→present; got {yoe:.2}");
    }
}
