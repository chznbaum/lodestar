//! Gap detection for the research-gaps pipeline step.
//! `detect_gaps` inspects a `Job` and returns the names of researchable JD-detail fields
//! that are still empty — the list of fields the `research-gaps` LLM step should look up.
#![allow(dead_code)]

use crate::job::Job;

/// The ordered list of researchable JD-detail fields.  Fields appear in this order in the
/// returned Vec — callers may rely on the order for prompt construction.
const RESEARCHABLE_FIELDS: &[&str] = &[
    "comp_low",
    "comp_high",
    "comp_currency",
    "comp_period",
    "comp_equity",
    "remote",
    "location_constraints",
    "visa_sponsorship",
    "relocation",
    "employment_type",
    "yoe_min",
    "tech_stack",
    "reports_to",
    "team",
    "countries",
];

/// Return the names of researchable JD-detail fields that are still empty on `job`.
/// A fully-populated job returns an empty Vec.  A bare stub returns nearly all of them.
pub fn detect_gaps(job: &Job) -> Vec<String> {
    let mut gaps = Vec::new();
    for &field in RESEARCHABLE_FIELDS {
        if is_gap(job, field) {
            gaps.push(field.to_string());
        }
    }
    gaps
}

fn is_gap(job: &Job, field: &str) -> bool {
    match field {
        "comp_low" => job.comp_low.is_none(),
        "comp_high" => job.comp_high.is_none(),
        "comp_currency" => job.comp_currency.is_none(),
        "comp_period" => job.comp_period.is_none(),
        "comp_equity" => job.comp_equity.is_none(),
        "remote" => job.remote.is_none(),
        "location_constraints" => job.location_constraints.is_none(),
        "visa_sponsorship" => job.visa_sponsorship.is_none(),
        "relocation" => job.relocation.is_none(),
        "employment_type" => job.employment_type.is_none(),
        "yoe_min" => job.yoe_min.is_none(),
        "tech_stack" => job.tech_stack.is_empty(),
        "reports_to" => job.reports_to.is_none(),
        "team" => job.team.is_none(),
        "countries" => job.countries.is_empty(),
        // Unknown field names are never gaps — future-proof defensiveness.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal all-None/empty Job for use in tests.
    fn bare_job() -> Job {
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
            fit_seniority: None,
            fit_skills: None,
            fit_comp: None,
            fit_arrangement: None,
            fit_domain: None,
            researched: vec![],
            status: None,
            jd_raw_file: None,
            jd_fetched: false,
        }
    }

    #[test]
    fn fully_populated_returns_no_gaps() {
        let job = Job {
            comp_low: Some(150000),
            comp_high: Some(200000),
            comp_currency: Some("USD".into()),
            comp_period: Some("annual".into()),
            comp_equity: Some("0.1-0.4%".into()),
            remote: Some("remote".into()),
            location_constraints: Some("US only".into()),
            visa_sponsorship: Some("not_offered".into()),
            relocation: Some("unspecified".into()),
            employment_type: Some("full_time".into()),
            yoe_min: Some(5),
            tech_stack: vec!["rust".into()],
            reports_to: Some("CTO".into()),
            team: Some("Platform".into()),
            countries: vec!["US".into()],
            ..bare_job()
        };
        assert!(detect_gaps(&job).is_empty());
    }

    #[test]
    fn partially_populated_excludes_set_fields_includes_gaps() {
        // comp_low, remote, tech_stack are set; everything else is empty.
        let job = Job {
            comp_low: Some(170000),
            remote: Some("hybrid".into()),
            tech_stack: vec!["rust".into()],
            ..bare_job()
        };
        let gaps = detect_gaps(&job);

        // Set fields must NOT appear.
        assert!(!gaps.contains(&"comp_low".to_string()), "comp_low should not be a gap");
        assert!(!gaps.contains(&"remote".to_string()), "remote should not be a gap");
        assert!(!gaps.contains(&"tech_stack".to_string()), "tech_stack should not be a gap");

        // Several empty fields MUST appear.
        assert!(gaps.contains(&"visa_sponsorship".to_string()), "visa_sponsorship should be a gap");
        assert!(gaps.contains(&"comp_currency".to_string()), "comp_currency should be a gap");
        assert!(gaps.contains(&"countries".to_string()), "countries should be a gap");
        assert!(gaps.contains(&"comp_high".to_string()), "comp_high should be a gap");
        assert!(gaps.contains(&"comp_period".to_string()), "comp_period should be a gap");
        assert!(gaps.contains(&"comp_equity".to_string()), "comp_equity should be a gap");
        assert!(gaps.contains(&"location_constraints".to_string()), "location_constraints should be a gap");
        assert!(gaps.contains(&"relocation".to_string()), "relocation should be a gap");
        assert!(gaps.contains(&"employment_type".to_string()), "employment_type should be a gap");
        assert!(gaps.contains(&"yoe_min".to_string()), "yoe_min should be a gap");
        assert!(gaps.contains(&"reports_to".to_string()), "reports_to should be a gap");
        assert!(gaps.contains(&"team".to_string()), "team should be a gap");
    }

    #[test]
    fn order_matches_researchable_fields_constant() {
        // detect_gaps must return fields in RESEARCHABLE_FIELDS order.
        let job = bare_job();
        let gaps = detect_gaps(&job);
        // All researchable fields are gaps on a bare job.
        assert_eq!(gaps.len(), RESEARCHABLE_FIELDS.len());
        for (gap, &expected) in gaps.iter().zip(RESEARCHABLE_FIELDS.iter()) {
            assert_eq!(gap.as_str(), expected);
        }
    }

    #[test]
    fn bare_job_reports_all_researchable_fields_as_gaps() {
        let job = bare_job();
        let gaps = detect_gaps(&job);
        // Every field in RESEARCHABLE_FIELDS should be reported.
        for &field in RESEARCHABLE_FIELDS {
            assert!(
                gaps.contains(&field.to_string()),
                "expected {field} to be a gap on a bare job"
            );
        }
    }
}
