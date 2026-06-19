//! The discovery chain: open a `checks/` run, then scrape → sanitize → structure-listings
//! (LLM) → pre-filter/dedup → write job stubs, and leave the run `awaiting_input` for the
//! selection gate. Orchestrates the Task 1–4 pieces behind the injected `Scraper`/`Llm`;
//! tested end-to-end with the test-suite fakes (zero spend).
//!
//! Chain-order note: §4.2 lists the pre-filter before structure-listings (to save tokens),
//! but reliable title/url extraction from arbitrary careers HTML needs the LLM — so we run
//! structure-listings first, then the deterministic filter over its output. The filter logic
//! is identical either way; only its position moved. A cheaper pre-LLM coarse narrowing can
//! be added later if token cost warrants.
// Consumed by the app command (Task 6); suppress dead-code until wired.
#![allow(dead_code)]

use crate::check::{get_check, write_check, Check};
use crate::config::{model_for, PipelineConfig};
use crate::job::{job_slug, list_jobs, write_job_stub, Job};
use crate::llm::Llm;
use crate::pipeline::filter::{prefilter, RawListing};
use crate::pipeline::runner::{now_iso, record_step, run_scrape_step};
use crate::profile::read_target_criteria;
use crate::prompts::{build_structure_listings_prompt, parse_structured_listings, StructuredListing};
use crate::sanitize::sanitize;
use crate::scraper::Scraper;
use std::collections::HashSet;
use std::path::Path;

/// `<today>-NNNN`, sequenced per day over existing `checks/` notes.
pub fn next_run_id(vault_path: &str, today: &str) -> Result<String, String> {
    let dir = Path::new(vault_path).join("checks");
    let count = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with(today) && n.ends_with(".md"))
                    .unwrap_or(false)
            })
            .count(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(format!("read checks dir: {e}")),
    };
    Ok(format!("{today}-{:04}", count + 1))
}

/// Run a discovery pass for one company: produce job stubs + a `checks/` run that ends
/// `awaiting_input`. Returns the final run. On any stage failure the run is marked `failed`
/// (so it never shows as perpetually running) and the error is propagated.
pub fn run_discovery<S: Scraper, L: Llm>(
    vault_path: &str,
    company_slug: &str,
    careers_url: &str,
    today: &str,
    cfg: &PipelineConfig,
    scraper: &S,
    llm: &L,
) -> Result<Check, String> {
    let base = Path::new(vault_path);
    std::fs::create_dir_all(base.join("checks")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(base.join("jobs")).map_err(|e| e.to_string())?;

    let run_id = next_run_id(vault_path, today)?;
    let run = Check {
        slug: run_id.clone(),
        kind: "job_check".into(),
        trigger: "manual".into(),
        status: "running".into(),
        started_at: Some(now_iso()),
        finished_at: None,
        duration: None,
        companies: vec![company_slug.to_string()],
        roles_found: 0,
        jds_fetched: 0,
        errors: 0,
        steps: vec![],
    };
    write_check(vault_path, &run)?;

    match discover_inner(vault_path, &run_id, company_slug, careers_url, today, cfg, scraper, llm) {
        Ok(roles_found) => {
            let mut final_run = get_check(vault_path.to_string(), run_id)?;
            final_run.roles_found = roles_found;
            final_run.status = "awaiting_input".into();
            final_run.finished_at = Some(now_iso());
            write_check(vault_path, &final_run)?;
            Ok(final_run)
        }
        Err(e) => {
            if let Ok(mut r) = get_check(vault_path.to_string(), run_id) {
                r.status = "failed".into();
                r.errors += 1;
                r.finished_at = Some(now_iso());
                let _ = write_check(vault_path, &r);
            }
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn discover_inner<S: Scraper, L: Llm>(
    vault_path: &str,
    run_id: &str,
    company_slug: &str,
    careers_url: &str,
    today: &str,
    cfg: &PipelineConfig,
    scraper: &S,
    llm: &L,
) -> Result<u32, String> {
    // 1. scrape (records careers-scrape, returns content)
    let scraped = run_scrape_step(vault_path, run_id, careers_url, company_slug, scraper)?;

    // 2. sanitize (script)
    let started = now_iso();
    let clean = sanitize(&scraped.content);
    record_step(vault_path, run_id, "sanitize", "script", company_slug, started, "ok", None, None)?;

    // 3. structure-listings (LLM): record ok/failed with the actual cost from usage.cost
    let started = now_iso();
    let prompt = build_structure_listings_prompt(&model_for(cfg, "structure-listings"), &clean);
    let listings: Vec<StructuredListing> = match llm.complete(&prompt) {
        Ok(resp) => {
            let cost = resp.cost_usd;
            match parse_structured_listings(&resp.content) {
                Ok(l) => {
                    record_step(vault_path, run_id, "structure-listings", "llm", company_slug, started, "ok", None, cost)?;
                    l
                }
                Err(e) => {
                    record_step(vault_path, run_id, "structure-listings", "llm", company_slug, started, "failed", Some(e.clone()), cost)?;
                    return Err(e);
                }
            }
        }
        Err(e) => {
            record_step(vault_path, run_id, "structure-listings", "llm", company_slug, started, "failed", Some(e.clone()), None)?;
            return Err(e);
        }
    };

    // 4. pre-filter + dedup (script). Drop url-less listings (unactionable + can't dedup),
    //    convert to the neutral RawListing, filter, then correlate kept urls back to the
    //    full structured listings for stub construction.
    let started = now_iso();
    let criteria = read_target_criteria(vault_path)?;
    let existing: HashSet<String> =
        list_jobs(vault_path.to_string())?.into_iter().filter_map(|j| j.url).collect();
    let with_url: Vec<StructuredListing> =
        listings.into_iter().filter(|l| l.url.is_some()).collect();
    let raws: Vec<RawListing> = with_url
        .iter()
        .map(|l| RawListing {
            title: l.title.clone(),
            url: l.url.clone().unwrap_or_default(),
            location: l.location.clone(),
        })
        .collect();
    let kept_urls: HashSet<String> =
        prefilter(raws, &criteria, &existing).into_iter().map(|r| r.url).collect();
    let selected: Vec<&StructuredListing> = with_url
        .iter()
        .filter(|l| l.url.as_deref().map(|u| kept_urls.contains(u)).unwrap_or(false))
        .collect();
    record_step(vault_path, run_id, "pre-filter", "script", company_slug, started, "ok", None, None)?;

    // 5. write job stubs (a slug collision is skipped + logged, never fails the whole run)
    for listing in &selected {
        let job = Job {
            slug: job_slug(&listing.title, company_slug),
            title: listing.title.clone(),
            company: Some(company_slug.to_string()),
            url: listing.url.clone(),
            classification: listing.classification.clone(),
            location: listing.location.clone(),
            comp_low: None,
            comp_high: None,
            comp_currency: None,
            comp_raw: None,
            date_posted: None,
            last_seen: Some(today.to_string()),
            ats: listing.ats.clone(),
            tech_stack: vec![],
            fit_score: None,
            status: Some("new".to_string()),
            skip_reason: None,
            jd_raw_file: None,
            jd_fetched: false,
        };
        if let Err(e) = write_job_stub(vault_path, &job) {
            eprintln!("skip stub {}: {e}", job.slug);
        }
    }

    Ok(selected.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::llm::tests::FakeLlm;
    use crate::scraper::tests::FakeScraper;

    #[test]
    fn next_run_id_increments_per_day() {
        let dir = std::env::temp_dir().join(format!("lodestar-runid-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        let vault = dir.to_str().unwrap().to_string();
        assert_eq!(next_run_id(&vault, "2026-06-18").unwrap(), "2026-06-18-0001");
        std::fs::write(dir.join("checks/2026-06-18-0001.md"), "---\nid: 2026-06-18-0001\n---\n").unwrap();
        assert_eq!(next_run_id(&vault, "2026-06-18").unwrap(), "2026-06-18-0002");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discovery_writes_filtered_stubs_and_awaiting_input_run() {
        let dir = std::env::temp_dir().join(format!("lodestar-disc-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("jobs")).unwrap();
        std::fs::create_dir_all(dir.join("checks")).unwrap();
        std::fs::create_dir_all(dir.join("profile")).unwrap();
        std::fs::write(
            dir.join("profile/target_criteria.md"),
            "---\ntype: target_criteria\nlocation_requirement: remote_only\nmatch_titles:\n  - engineer\n---\n",
        )
        .unwrap();
        let vault = dir.to_str().unwrap().to_string();

        let scraper = FakeScraper { content: "<p>careers</p>".into(), credits: 5 };
        let llm = FakeLlm {
            reply: r#"[{"title":"Senior Engineer","url":"https://co/1","location":"Remote","classification":"senior-ic"},
                       {"title":"Real Estate Agent","url":"https://co/2","location":"Remote"}]"#
                .into(),
            cost_usd: 0.02,
        };
        let run = run_discovery(
            &vault, "acme", "https://co/careers", "2026-06-18", &default_config(), &scraper, &llm,
        )
        .unwrap();

        assert_eq!(run.status, "awaiting_input");
        assert_eq!(run.companies, vec!["acme".to_string()]);
        assert_eq!(run.roles_found, 1); // agent filtered out
        assert!(run.steps.iter().any(|s| s.stage == "careers-scrape" && s.cost == Some(5.0)));
        assert!(run.steps.iter().any(|s| s.stage == "structure-listings" && s.cost == Some(0.02)));
        assert!(dir.join("jobs/senior-engineer-acme.md").exists());
        assert!(!dir.join("jobs/real-estate-agent-acme.md").exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
