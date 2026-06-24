//! Pure pre-filter for discovered listings: a recall-oriented title match against the user's
//! `match_titles` and dedup-by-url against existing jobs. Work arrangement is intentionally NOT
//! filtered here — onsite/hybrid listings surface and are scored softly by fit (`arrangement_fit`),
//! and the user decides per-role whether a compromise is worth actioning. Precision is deferred to
//! fit-ranking — this stage only excludes clear non-matches (e.g. "Real Estate Agent").
// Consumed by the discovery chain (Task 5); suppress dead-code until wired.
#![allow(dead_code)]

use crate::profile::TargetCriteria;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct RawListing {
    pub title: String,
    pub url: String,
    pub location: Option<String>,
}

/// Recall match: case-insensitive — a listing matches if its title contains ANY `match_title`.
fn title_matches(title: &str, match_titles: &[String]) -> bool {
    let t = title.to_lowercase();
    match_titles.iter().any(|m| t.contains(&m.to_lowercase()))
}

/// Narrow raw listings to deduped, on-target candidates. The URL is a job's identity, so we
/// drop URLs already in `existing_urls` *and* collapse duplicate URLs within this batch
/// (first occurrence wins) — a posting can surface twice in one scrape. Also drops titles that
/// match no `match_title` (clear non-matches). Work arrangement is intentionally NOT filtered —
/// onsite/hybrid listings surface and are scored softly by fit; the user decides per-role. Input
/// order is preserved.
pub fn prefilter(
    listings: Vec<RawListing>,
    criteria: &TargetCriteria,
    existing_urls: &HashSet<String>,
) -> Vec<RawListing> {
    let mut seen: HashSet<String> = HashSet::new();
    listings
        .into_iter()
        .filter(|l| !existing_urls.contains(&l.url))
        .filter(|l| seen.insert(l.url.clone())) // within-batch dedup-by-url: keep first occurrence
        .filter(|l| title_matches(&l.title, &criteria.match_titles))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crit() -> TargetCriteria {
        TargetCriteria {
            match_titles: vec![
                "founding engineer".into(),
                "ai engineer".into(),
                "software engineer".into(),
            ],
            target_titles: vec![],
            work_arrangements: vec!["remote".into()],
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
            fit_weights: crate::profile::FitWeights::default(),
            current_location: None,
            preferred_locations: vec![],
        }
    }
    fn listing(t: &str, u: &str, loc: Option<&str>) -> RawListing {
        RawListing {
            title: t.into(),
            url: u.into(),
            location: loc.map(String::from),
        }
    }

    #[test]
    fn keeps_recall_matches_drops_clear_nonmatches() {
        let ls = vec![
            listing("Senior Software Engineer", "u1", Some("Remote (US)")),
            listing("Forward-Deployed AI Engineer", "u2", Some("Remote")),
            listing("Real Estate Agent", "u3", Some("Remote")),
        ];
        let kept = prefilter(ls, &crit(), &HashSet::new());
        let urls: Vec<_> = kept.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, vec!["u1", "u2"]); // agent dropped
    }

    #[test]
    fn dedup_drops_known_keeps_onsite_and_unknown() {
        // Arrangement is NOT filtered at discovery — onsite & unknown-location listings surface
        // (fit scores them softly); only the already-known url is dropped.
        let mut existing = HashSet::new();
        existing.insert("u1".to_string());
        let ls = vec![
            listing("Software Engineer", "u1", Some("Remote")), // known url -> drop
            listing("Software Engineer", "u2", Some("New York, NY")), // onsite -> KEPT
            listing("AI Engineer", "u3", None),                 // unknown loc -> KEPT
            listing("AI Engineer", "u4", Some("Remote (US)")),  // remote -> KEPT
        ];
        let kept = prefilter(ls, &crit(), &existing);
        assert_eq!(
            kept.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(),
            vec!["u2", "u3", "u4"]
        );
    }

    #[test]
    fn dedups_duplicate_urls_within_batch() {
        // The same posting can surface twice in one scrape (pagination overlap, double-parse).
        // The URL is identity, so the batch must collapse duplicates against itself — not just
        // against on-disk URLs. The first occurrence wins; order is otherwise preserved.
        let ls = vec![
            listing("Software Engineer", "u1", Some("Remote")),
            listing("AI Engineer", "u2", Some("Remote")),
            listing("Software Engineer", "u1", Some("Remote")), // dup of the first by url
        ];
        let kept = prefilter(ls, &crit(), &HashSet::new());
        assert_eq!(
            kept.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(),
            vec!["u1", "u2"]
        );
    }
}
