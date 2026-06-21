//! Hard-filter and soft-scoring layers of the fit rubric (§4.10).
//!
//! **Hard layer** — `hard_filters` returns fired `Flag`s (each check fires at most one).
//! Recall-safe: a flag fires ONLY on a *known* conflict. Unknown fields on either
//! side are skipped — a false dealbreaker is worse than a missed one.
//!
//! **Soft layer** — five 0–1 sub-scores combined into a 0–100 `score` via `score_fit`.
//! Any dealbreaker flag collapses the score to 0 (hard no — not a cap).
//!
//! Decay/penalty calibration constants (tunable):
//! - Within-track seniority distance: 0→1.0, 1→0.6, 2→0.3, ≥3→0.1
//! - Cross-track seniority mismatch: 0.1
//! - Arrangement mismatch (known but not in list): 0.15
//! - Skills split (required vs preferred): 0.8 / 0.2
#![allow(dead_code)]

use crate::competency::CompetencyIndex;
use crate::job::Job;
use crate::profile::TargetCriteria;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FlagLevel {
    Dealbreaker,
    Caution,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Flag {
    pub check: String,
    pub level: FlagLevel,
    pub detail: String,
}

fn db(check: &str, detail: impl Into<String>) -> Flag {
    Flag {
        check: check.to_string(),
        level: FlagLevel::Dealbreaker,
        detail: detail.into(),
    }
}

fn caution(check: &str, detail: impl Into<String>) -> Flag {
    Flag {
        check: check.to_string(),
        level: FlagLevel::Caution,
        detail: detail.into(),
    }
}

// ── Soft scoring ────────────────────────────────────────────────────────────

/// Output of `score_fit`: the five 0–1 sub-scores, the fired flags, and the
/// combined 0–100 score (0 when any dealbreaker fires).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FitBreakdown {
    pub seniority: f64,
    pub skills: f64,
    pub comp: f64,
    pub arrangement: f64,
    pub domain: f64,
    pub flags: Vec<Flag>,
    pub score: i64,
}

/// IC vs. management axis for the two-track seniority model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Track {
    Ic,
    Management,
}

/// Map a valid level string to its (track, within-track rank).
/// Returns `None` for unknown levels so callers can treat them as neutral.
fn level_track(level: &str) -> Option<(Track, usize)> {
    match level {
        "junior" => Some((Track::Ic, 0)),
        "mid" => Some((Track::Ic, 1)),
        "senior" => Some((Track::Ic, 2)),
        "front-line-mgmt" => Some((Track::Management, 0)),
        "middle-mgmt" => Some((Track::Management, 1)),
        "dept-head" => Some((Track::Management, 2)),
        "vp" => Some((Track::Management, 3)),
        "c-suite" => Some((Track::Management, 4)),
        _ => None,
    }
}

/// Seniority fit sub-score (0–1).
///
/// 1. Base level score from job level vs. targeted levels.
/// 2. YOE reducer: scales base down proportionally when `candidate_yoe < yoe_min`.
///    Unknown `yoe_min` or unknown candidate YOE (0.0) → no reduction.
pub(crate) fn seniority_fit(
    job_level: Option<&str>,
    targets: &[String],
    job_yoe_min: Option<i64>,
    candidate_yoe: f64,
) -> f64 {
    // --- 1. Base level score ---
    let base = match job_level.and_then(level_track) {
        None => 0.5, // unknown level → neutral
        Some((job_track, job_rank)) => {
            // Exact match?
            if job_level.is_some_and(|l| targets.iter().any(|t| t == l)) {
                1.0
            } else {
                // Collect valid-level targets and their tracks.
                let target_tracks: Vec<(Track, usize)> =
                    targets.iter().filter_map(|t| level_track(t)).collect();

                if target_tracks.is_empty() {
                    // No seniority target set → neutral (don't penalize an unspecified preference).
                    0.5
                } else {
                    // Is job's track among targeted tracks?
                    let same_track_targets: Vec<usize> = target_tracks
                        .iter()
                        .filter(|(t, _)| *t == job_track)
                        .map(|(_, r)| *r)
                        .collect();

                    if same_track_targets.is_empty() {
                        0.1 // cross-track mismatch
                    } else {
                        // Nearest same-track target by rank distance.
                        let min_dist = same_track_targets
                            .iter()
                            .map(|&r| (r as isize - job_rank as isize).unsigned_abs())
                            .min()
                            .unwrap_or(usize::MAX);
                        match min_dist {
                            0 => 1.0,
                            1 => 0.6,
                            2 => 0.3,
                            _ => 0.1,
                        }
                    }
                }
            }
        }
    };

    // --- 2. YOE reducer ---
    let factor = if let Some(y) = job_yoe_min {
        if y > 0 && candidate_yoe > 0.0 {
            (candidate_yoe / y as f64).min(1.0)
        } else {
            1.0
        }
    } else {
        1.0
    };

    base * factor
}

/// Skills fit sub-score (0–1).
///
/// Coverage = matched / total per list. Required dominates (0.8 / 0.2).
/// Both empty → 0.5 (neutral).
pub(crate) fn skills_fit(
    required: &[String],
    preferred: &[String],
    idx: &CompetencyIndex,
) -> f64 {
    let coverage = |list: &[String]| -> f64 {
        if list.is_empty() {
            return 0.0; // sentinel; callers check emptiness before using
        }
        let matched = list.iter().filter(|s| idx.matches(s)).count();
        matched as f64 / list.len() as f64
    };

    match (required.is_empty(), preferred.is_empty()) {
        (true, true) => 0.5,
        (false, true) => coverage(required),
        (true, false) => coverage(preferred),
        (false, false) => 0.8 * coverage(required) + 0.2 * coverage(preferred),
    }
}

/// Comp fit sub-score (0–1).
///
/// Compares the job's band ceiling (`high`) against the candidate's floor and target.
/// Unknown comp or floor → 0.5 (neutral).
pub(crate) fn comp_fit(high: Option<i64>, floor: Option<i64>, target: Option<i64>) -> f64 {
    match (high, floor, target) {
        (Some(h), Some(f), Some(t)) if t > f => {
            ((h - f) as f64 / (t - f) as f64).clamp(0.0, 1.0)
        }
        (Some(h), Some(f), _) => {
            if h >= f {
                1.0
            } else {
                0.0
            }
        }
        _ => 0.5,
    }
}

/// Arrangement fit sub-score (0–1).
///
/// `None` job remote → 0.5; match in list → 1.0; known and not in list → 0.15.
pub(crate) fn arrangement_fit(job_remote: Option<&str>, work_arrangements: &[String]) -> f64 {
    match job_remote {
        None => 0.5,
        Some(r) if work_arrangements.iter().any(|a| a == r) => 1.0,
        Some(_) => 0.15,
    }
}

/// Domain fit sub-score (0–1).
///
/// Avoid hit → 0.0; preferred hit → 1.0; preferred empty → 0.5; else 0.4.
pub(crate) fn domain_fit(
    company_domains: &[String],
    preferred: &[String],
    avoid: &[String],
) -> f64 {
    if company_domains.iter().any(|d| avoid.contains(d)) {
        return 0.0;
    }
    if preferred.is_empty() {
        return 0.5;
    }
    if company_domains.iter().any(|d| preferred.contains(d)) {
        1.0
    } else {
        0.4
    }
}

/// Compute the full fit breakdown for a job against a candidate profile.
///
/// Sub-scores are combined via `p.fit_weights`; any dealbreaker collapses `score` to 0.
pub fn score_fit(
    job: &Job,
    p: &TargetCriteria,
    company_domains: &[String],
    company_screening: Option<&str>,
    comps: &CompetencyIndex,
    candidate_yoe: f64,
) -> FitBreakdown {
    let w = &p.fit_weights;

    let seniority = seniority_fit(
        job.level.as_deref(),
        &p.target_levels,
        job.yoe_min,
        candidate_yoe,
    );
    let skills = skills_fit(&job.required_skills, &job.preferred_skills, comps);
    let comp = comp_fit(job.comp_high.or(job.comp_low), p.comp_floor, p.comp_target);
    let arrangement = arrangement_fit(job.remote.as_deref(), &p.work_arrangements);
    let domain = domain_fit(company_domains, &p.preferred_domains, &p.avoid_domains);

    let raw = 100.0
        * (w.seniority * seniority
            + w.skills * skills
            + w.comp * comp
            + w.arrangement * arrangement
            + w.domain * domain);

    let flags = hard_filters(job, p, company_screening);

    let score = if flags.iter().any(|f| f.level == FlagLevel::Dealbreaker) {
        0
    } else {
        raw.round() as i64
    };

    FitBreakdown {
        seniority,
        skills,
        comp,
        arrangement,
        domain,
        flags,
        score,
    }
}

// ── Hard filters ─────────────────────────────────────────────────────────────

/// Returns all fired flags for the given job/profile pair.
/// `company_screening` is the company's derived screening value
/// (`"dealbreaker"` | `"caution"` | `None`).
pub fn hard_filters(
    job: &Job,
    p: &TargetCriteria,
    company_screening: Option<&str>,
) -> Vec<Flag> {
    let mut flags: Vec<Flag> = Vec::new();

    // Convenience: is this a non-remote (onsite or hybrid) role?
    // Unknown `remote` (None) → false → treated as "possibly remote" → no remote flag.
    let non_remote = matches!(job.remote.as_deref(), Some("onsite") | Some("hybrid"));

    // 1. Company screening.
    match company_screening {
        Some("dealbreaker") => flags.push(db("company", "company is marked dealbreaker")),
        Some("caution") => flags.push(caution("company", "company is marked caution")),
        _ => {}
    }

    // 2. (arrangement soft-scoring — Task 5, not a hard dealbreaker here)

    // 3. Work authorization (Dealbreaker) — non-remote only.
    // Skip entirely for fully-remote roles (no jurisdiction constraint).
    // Run when the candidate's authorized countries are listed OR they've explicitly declared they
    // require sponsorship — both are real signals. Only when the list is empty AND they don't
    // require sponsorship is eligibility genuinely unknown → skip.
    if non_remote && (!p.work_authorization.is_empty() || p.requires_sponsorship) {
        let authorized = !job.countries.is_empty()
            && job.countries.iter().any(|c| p.work_authorization.contains(c));

        // Needs sponsorship when:
        //   - role's country is known and candidate is not authorized, OR
        //   - country unknown but candidate broadly requires sponsorship (they've told us).
        let needs_sponsorship = !authorized
            && (!job.countries.is_empty() || p.requires_sponsorship);

        if needs_sponsorship && job.visa_sponsorship.as_deref() == Some("not_offered") {
            flags.push(db(
                "work_authorization",
                format!(
                    "not authorized in {:?}; sponsorship not offered",
                    job.countries
                ),
            ));
        }
    }

    // 4. Relocation (Dealbreaker) — non-remote only, role metros known, candidate location known.
    // Skip if remote. Skip if metros unknown. Skip if candidate location entirely unknown.
    if non_remote
        && !job.metros.is_empty()
        && (p.current_location.is_some() || !p.preferred_locations.is_empty())
    {
        let home = p.current_location.as_deref();
        let is_local = home.is_some_and(|h| job.metros.iter().any(|m| m == h));

        if !is_local {
            let on_accept_list = job
                .metros
                .iter()
                .any(|m| p.preferred_locations.contains(m));
            let ok = job.relocation.as_deref() == Some("offered")
                && p.open_to_relocation
                && on_accept_list;
            if !ok {
                flags.push(db(
                    "relocation",
                    format!(
                        "role in {:?}; not local and relocation terms not met",
                        job.metros
                    ),
                ));
            }
        }
    }

    // 5. Comp floor (Dealbreaker).
    // Only fires when both a floor and a band ceiling (comp_high, falling back to comp_low) are
    // known, the currency matches, and the period is (or defaults to) annual.
    if let Some(floor) = p.comp_floor {
        if let Some(high) = job.comp_high.or(job.comp_low) {
            let same_ccy = job.comp_currency.is_none()
                || job.comp_currency.as_deref() == p.comp_currency.as_deref();
            // Unknown period → assume annual (recall-safe: flag only on known-annual mismatch).
            let annual = job
                .comp_period
                .as_deref()
                .is_none_or(|x| x == "annual");

            if same_ccy && annual && high < floor {
                flags.push(db(
                    "comp_floor",
                    format!("band tops out at {high}, floor {floor}"),
                ));
            }
        }
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::FitWeights;

    /// Construct a `Job` with all optional fields set to None/empty.
    fn base_job() -> Job {
        Job {
            slug: "test-job".to_string(),
            title: "Test Job".to_string(),
            company: None,
            url: None,
            level: None,
            location: None,
            comp_low: None,
            comp_high: None,
            comp_currency: None,
            comp_raw: None,
            comp_period: None,
            comp_equity: None,
            employment_type: None,
            yoe_min: None,
            yoe_max: None,
            tech_stack: vec![],
            required_skills: vec![],
            preferred_skills: vec![],
            reports_to: None,
            team: None,
            remote: None,
            location_constraints: None,
            visa_sponsorship: None,
            relocation: None,
            countries: vec![],
            metros: vec![],
            application_url: None,
            date_posted: None,
            last_seen: None,
            ats: None,
            fit_score: None,
            researched: vec![],
            status: None,
            skip_reason: None,
            jd_raw_file: None,
            jd_fetched: false,
        }
    }

    /// Construct a `TargetCriteria` with all optional fields set to None/empty/false.
    fn base_profile() -> TargetCriteria {
        TargetCriteria {
            match_titles: vec![],
            target_titles: vec![],
            work_arrangements: vec![],
            target_levels: vec![],
            comp_floor: None,
            comp_target: None,
            comp_currency: None,
            employment_types: vec![],
            open_to_relocation: false,
            work_authorization: vec![],
            requires_sponsorship: false,
            preferred_domains: vec![],
            avoid_domains: vec![],
            fit_weights: FitWeights::default(),
            current_location: None,
            preferred_locations: vec![],
        }
    }

    // --- Check 1: Company screening ---

    #[test]
    fn company_dealbreaker_screening_fires_dealbreaker() {
        let job = base_job();
        let profile = base_profile();
        let flags = hard_filters(&job, &profile, Some("dealbreaker"));
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].check, "company");
        assert_eq!(flags[0].level, FlagLevel::Dealbreaker);
    }

    #[test]
    fn company_caution_screening_fires_caution_not_dealbreaker() {
        let job = base_job();
        let profile = base_profile();
        let flags = hard_filters(&job, &profile, Some("caution"));
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].check, "company");
        assert_eq!(flags[0].level, FlagLevel::Caution);
        // Must NOT be a Dealbreaker
        assert_ne!(flags[0].level, FlagLevel::Dealbreaker);
    }

    #[test]
    fn company_no_screening_no_flag() {
        let flags = hard_filters(&base_job(), &base_profile(), None);
        assert!(flags.iter().all(|f| f.check != "company"));
    }

    // --- Check 3: Work authorization ---

    #[test]
    fn work_auth_onsite_de_role_us_auth_no_sponsorship_fires() {
        let job = Job {
            remote: Some("onsite".to_string()),
            countries: vec!["DE".to_string()],
            visa_sponsorship: Some("not_offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            work_authorization: vec!["US".to_string()],
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().any(|f| f.check == "work_authorization" && f.level == FlagLevel::Dealbreaker),
            "expected work_authorization dealbreaker, got: {flags:?}"
        );
    }

    #[test]
    fn work_auth_sponsorship_offered_no_flag() {
        let job = Job {
            remote: Some("onsite".to_string()),
            countries: vec!["DE".to_string()],
            visa_sponsorship: Some("offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            work_authorization: vec!["US".to_string()],
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "work_authorization"),
            "sponsorship offered should suppress work_authorization flag"
        );
    }

    #[test]
    fn work_auth_fully_remote_role_no_flag() {
        // Remote role → skip work authorization check entirely
        let job = Job {
            remote: Some("remote".to_string()),
            countries: vec!["DE".to_string()],
            visa_sponsorship: Some("not_offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            work_authorization: vec!["US".to_string()],
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "work_authorization"),
            "fully remote role should not fire work_authorization flag"
        );
    }

    #[test]
    fn work_auth_empty_profile_auth_no_flag() {
        // work_authorization: [] → candidate auth unknown → skip (recall-safe)
        let job = Job {
            remote: Some("onsite".to_string()),
            countries: vec!["DE".to_string()],
            visa_sponsorship: Some("not_offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            work_authorization: vec![], // empty → unknown
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "work_authorization"),
            "empty work_authorization should not fire flag"
        );
    }

    #[test]
    fn work_auth_requires_sponsorship_no_authlist_no_offer_fires() {
        // Candidate requires sponsorship and listed no authorized countries; a non-remote role
        // that doesn't offer sponsorship is still a dealbreaker — not a pass.
        let job = Job {
            remote: Some("onsite".to_string()),
            visa_sponsorship: Some("not_offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            work_authorization: vec![],
            requires_sponsorship: true,
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().any(|f| f.check == "work_authorization" && f.level == FlagLevel::Dealbreaker),
            "requires_sponsorship + not_offered + empty auth list should fire, got: {flags:?}"
        );
    }

    // --- Check 4: Relocation ---

    #[test]
    fn relocation_onsite_non_local_no_preferred_fires_dealbreaker() {
        let job = Job {
            remote: Some("onsite".to_string()),
            metros: vec!["austin-round-rock-san-marcos-tx".to_string()],
            ..base_job()
        };
        let profile = TargetCriteria {
            current_location: Some("new-york-newark-jersey-city-ny-nj-pa".to_string()),
            preferred_locations: vec![],
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().any(|f| f.check == "relocation" && f.level == FlagLevel::Dealbreaker),
            "expected relocation dealbreaker, got: {flags:?}"
        );
    }

    #[test]
    fn relocation_metro_on_accept_list_relocation_offered_open_no_flag() {
        let metro = "austin-round-rock-san-marcos-tx".to_string();
        let job = Job {
            remote: Some("onsite".to_string()),
            metros: vec![metro.clone()],
            relocation: Some("offered".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            current_location: Some("new-york-newark-jersey-city-ny-nj-pa".to_string()),
            preferred_locations: vec![metro],
            open_to_relocation: true,
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "relocation"),
            "relocation offered + open + on accept list should not fire relocation flag"
        );
    }

    #[test]
    fn relocation_metro_is_current_location_no_flag() {
        let metro = "austin-round-rock-san-marcos-tx".to_string();
        let job = Job {
            remote: Some("onsite".to_string()),
            metros: vec![metro.clone()],
            ..base_job()
        };
        let profile = TargetCriteria {
            current_location: Some(metro),
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "relocation"),
            "local role should not fire relocation flag"
        );
    }

    #[test]
    fn relocation_candidate_location_entirely_unset_no_flag() {
        // current_location: None + preferred_locations: [] → location unknown → skip
        let job = Job {
            remote: Some("onsite".to_string()),
            metros: vec!["austin-round-rock-san-marcos-tx".to_string()],
            ..base_job()
        };
        let profile = TargetCriteria {
            current_location: None,
            preferred_locations: vec![],
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "relocation"),
            "unknown candidate location should not fire relocation flag"
        );
    }

    // --- Check 5: Comp floor ---

    #[test]
    fn comp_floor_band_top_below_floor_fires_dealbreaker() {
        let job = Job {
            comp_high: Some(150_000),
            comp_currency: Some("USD".to_string()),
            comp_period: Some("annual".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            comp_floor: Some(180_000),
            comp_currency: Some("USD".to_string()),
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().any(|f| f.check == "comp_floor" && f.level == FlagLevel::Dealbreaker),
            "expected comp_floor dealbreaker, got: {flags:?}"
        );
    }

    #[test]
    fn comp_floor_band_top_meets_floor_no_flag() {
        let job = Job {
            comp_high: Some(200_000),
            comp_currency: Some("USD".to_string()),
            comp_period: Some("annual".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            comp_floor: Some(180_000),
            comp_currency: Some("USD".to_string()),
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(flags.iter().all(|f| f.check != "comp_floor"));
    }

    #[test]
    fn comp_floor_different_currency_no_flag() {
        // Currencies don't match → can't compare → no flag (recall-safe)
        let job = Job {
            comp_high: Some(150_000),
            comp_currency: Some("GBP".to_string()),
            comp_period: Some("annual".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            comp_floor: Some(180_000),
            comp_currency: Some("USD".to_string()),
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "comp_floor"),
            "different currencies should not fire comp_floor flag"
        );
    }

    // ── Soft scoring tests ────────────────────────────────────────────────

    fn rust_idx() -> CompetencyIndex {
        use crate::competency::Competency;
        let comps = vec![Competency {
            slug: "rust".to_string(),
            name: "Rust".to_string(),
            aliases: vec![],
        }];
        CompetencyIndex::build(&comps)
    }

    // --- seniority_fit ---

    #[test]
    fn seniority_exact_match_in_targets() {
        // "senior" in [senior, dept-head] → 1.0
        assert_eq!(
            seniority_fit(Some("senior"), &["senior".into(), "dept-head".into()], None, 0.0),
            1.0
        );
    }

    #[test]
    fn seniority_exact_match_mgmt() {
        // "dept-head" in [senior, dept-head] → 1.0
        assert_eq!(
            seniority_fit(Some("dept-head"), &["senior".into(), "dept-head".into()], None, 0.0),
            1.0
        );
    }

    #[test]
    fn seniority_within_track_dist1() {
        // "mid" vs [senior] → IC track, dist 1 → 0.6
        assert_eq!(
            seniority_fit(Some("mid"), &["senior".into()], None, 0.0),
            0.6
        );
    }

    #[test]
    fn seniority_cross_track() {
        // "front-line-mgmt" vs [senior] (IC-only targets) → cross-track → 0.1
        assert_eq!(
            seniority_fit(Some("front-line-mgmt"), &["senior".into()], None, 0.0),
            0.1
        );
    }

    #[test]
    fn seniority_unknown_level() {
        // unknown level → 0.5
        assert_eq!(
            seniority_fit(None, &["senior".into()], None, 0.0),
            0.5
        );
    }

    #[test]
    fn seniority_empty_targets_is_neutral() {
        // No target_levels set → profile-side unknown → neutral 0.5 (not a cross-track penalty).
        assert_eq!(seniority_fit(Some("senior"), &[], None, 0.0), 0.5);
        assert_eq!(seniority_fit(Some("dept-head"), &[], None, 0.0), 0.5);
    }

    #[test]
    fn seniority_yoe_reducer_scales_down() {
        // "senior" exact match, base=1.0; yoe_min=10, candidate=5 → factor=0.5 → 0.5
        assert_eq!(
            seniority_fit(Some("senior"), &["senior".into()], Some(10), 5.0),
            0.5
        );
    }

    #[test]
    fn seniority_yoe_reducer_unknown_candidate_no_reduction() {
        // candidate_yoe=0.0 means unknown → no reduction → 1.0
        assert_eq!(
            seniority_fit(Some("senior"), &["senior".into()], Some(10), 0.0),
            1.0
        );
    }

    #[test]
    fn seniority_yoe_no_yoe_min_no_reduction() {
        // no yoe_min → factor=1.0 → no reduction
        assert_eq!(
            seniority_fit(Some("senior"), &["senior".into()], None, 5.0),
            1.0
        );
    }

    // --- skills_fit ---

    #[test]
    fn skills_required_partial_and_preferred_miss() {
        // required: [rust(match), go(miss)]; preferred: [k8s(miss)]
        // req_cov = 0.5, pref_cov = 0.0 → 0.8*0.5 + 0.2*0.0 = 0.4
        let idx = rust_idx();
        let score = skills_fit(
            &["rust".into(), "go".into()],
            &["k8s".into()],
            &idx,
        );
        assert!((score - 0.4).abs() < 1e-9, "expected 0.4, got {score}");
    }

    #[test]
    fn skills_all_required_matched_no_preferred() {
        let idx = rust_idx();
        assert_eq!(
            skills_fit(&["rust".into()], &[], &idx),
            1.0
        );
    }

    #[test]
    fn skills_both_empty() {
        let idx = rust_idx();
        assert_eq!(skills_fit(&[], &[], &idx), 0.5);
    }

    // --- comp_fit ---

    #[test]
    fn comp_at_target() {
        // high=220k, floor=180k, target=220k → (220k-180k)/(220k-180k)=1.0
        assert_eq!(comp_fit(Some(220_000), Some(180_000), Some(220_000)), 1.0);
    }

    #[test]
    fn comp_midway() {
        // high=200k, floor=180k, target=220k → (200k-180k)/(220k-180k) = 20/40 = 0.5
        assert_eq!(comp_fit(Some(200_000), Some(180_000), Some(220_000)), 0.5);
    }

    #[test]
    fn comp_unknown() {
        assert_eq!(comp_fit(None, None, None), 0.5);
    }

    // --- arrangement_fit ---

    #[test]
    fn arrangement_remote_in_list() {
        assert_eq!(arrangement_fit(Some("remote"), &["remote".into()]), 1.0);
    }

    #[test]
    fn arrangement_onsite_not_in_list() {
        assert_eq!(arrangement_fit(Some("onsite"), &["remote".into()]), 0.15);
    }

    #[test]
    fn arrangement_none_neutral() {
        assert_eq!(arrangement_fit(None, &["remote".into()]), 0.5);
    }

    // --- domain_fit ---

    #[test]
    fn domain_preferred_hit() {
        assert_eq!(
            domain_fit(&["dev_tools".into()], &["dev_tools".into()], &[]),
            1.0
        );
    }

    #[test]
    fn domain_avoid_hit() {
        assert_eq!(
            domain_fit(&["gambling".into()], &["dev_tools".into()], &["gambling".into()]),
            0.0
        );
    }

    #[test]
    fn domain_no_prefs() {
        assert_eq!(domain_fit(&["fintech".into()], &[], &[]), 0.5);
    }

    // --- score_fit ---

    #[test]
    fn score_fit_clean_job_combines_sub_scores() {
        let idx = rust_idx();
        let job = Job {
            level: Some("senior".into()),
            required_skills: vec!["rust".into()],
            comp_high: Some(220_000),
            comp_currency: Some("USD".into()),
            comp_period: Some("annual".into()),
            remote: Some("remote".into()),
            ..base_job()
        };
        let profile = TargetCriteria {
            target_levels: vec!["senior".into()],
            comp_floor: Some(180_000),
            comp_target: Some(220_000),
            comp_currency: Some("USD".into()),
            work_arrangements: vec!["remote".into()],
            preferred_domains: vec!["dev_tools".into()],
            ..base_profile()
        };
        let bd = score_fit(&job, &profile, &["dev_tools".into()], None, &idx, 7.0);
        // No dealbreakers → score > 0
        assert!(bd.score > 0, "expected positive score, got {}", bd.score);
        assert!(bd.flags.iter().all(|f| f.level != FlagLevel::Dealbreaker));
        // Verify it equals round(100 * weighted sum) with default weights
        let w = &profile.fit_weights;
        let expected = (100.0
            * (w.seniority * bd.seniority
                + w.skills * bd.skills
                + w.comp * bd.comp
                + w.arrangement * bd.arrangement
                + w.domain * bd.domain))
            .round() as i64;
        assert_eq!(bd.score, expected);
    }

    #[test]
    fn score_fit_dealbreaker_collapses_to_zero() {
        // A job with comp below floor triggers comp_floor dealbreaker → score 0
        // even though other sub-scores are strong.
        let idx = rust_idx();
        let job = Job {
            level: Some("senior".into()),
            required_skills: vec!["rust".into()],
            comp_high: Some(150_000), // below floor
            comp_currency: Some("USD".into()),
            comp_period: Some("annual".into()),
            remote: Some("remote".into()),
            ..base_job()
        };
        let profile = TargetCriteria {
            target_levels: vec!["senior".into()],
            comp_floor: Some(180_000),
            comp_currency: Some("USD".into()),
            work_arrangements: vec!["remote".into()],
            ..base_profile()
        };
        let bd = score_fit(&job, &profile, &[], None, &idx, 10.0);
        assert_eq!(
            bd.score, 0,
            "dealbreaker should collapse score to 0, got {}",
            bd.score
        );
        assert!(
            bd.flags.iter().any(|f| f.level == FlagLevel::Dealbreaker),
            "expected a dealbreaker flag, got: {:?}",
            bd.flags
        );
    }
}
