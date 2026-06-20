//! Deterministic hard-filter layer of the fit rubric (§4.10).
//! Returns a list of fired `Flag`s (each check fires at most one).
//! Recall-safe: a flag fires ONLY on a *known* conflict. Unknown fields on either
//! side are skipped — a false dealbreaker is worse than a missed one.
//! Soft weighted scoring + the dealbreaker→fit_score 0 rule live in Task 5.
#![allow(dead_code)]

use crate::job::Job;
use crate::profile::TargetCriteria;

#[derive(Debug, Clone, PartialEq)]
pub enum FlagLevel {
    Dealbreaker,
    Caution,
}

#[derive(Debug, Clone, PartialEq)]
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

    // 2. Remote (Dealbreaker).
    // Only fires when profile is remote-only AND the role is known non-remote.
    if p.remote_only && non_remote {
        flags.push(db(
            "remote",
            format!(
                "{} role; user is remote-only",
                job.remote.as_deref().unwrap_or("onsite")
            ),
        ));
    }

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
            remote_only: false,
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

    // --- Check 2: Remote ---

    #[test]
    fn remote_onsite_remote_only_profile_fires_dealbreaker() {
        let job = Job {
            remote: Some("onsite".to_string()),
            ..base_job()
        };
        let profile = TargetCriteria {
            remote_only: true,
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].check, "remote");
        assert_eq!(flags[0].level, FlagLevel::Dealbreaker);
    }

    #[test]
    fn remote_unknown_remote_only_profile_no_flag() {
        // remote: None → unknown → no remote flag even when profile is remote_only
        let job = base_job(); // remote: None
        let profile = TargetCriteria {
            remote_only: true,
            ..base_profile()
        };
        let flags = hard_filters(&job, &profile, None);
        assert!(
            flags.iter().all(|f| f.check != "remote"),
            "should not fire remote flag when job remote is unknown"
        );
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
}
