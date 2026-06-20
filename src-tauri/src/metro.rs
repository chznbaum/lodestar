//! Alias-aware metro index: maps a free-text location string to one of the user's
//! metro notes (name / alias / slug), case-insensitively via `note::slugify` normalization.
#![allow(dead_code)]

use crate::note::{self, slugify, split_frontmatter};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct Metro {
    pub slug: String,
    pub name: String,
    pub country: String,
    pub aliases: Vec<String>,
}

#[derive(Deserialize)]
struct Front {
    name: Option<String>,
    country: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

fn parse_metro(slug: &str, text: &str) -> Result<Metro, String> {
    let (fm, _) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    Ok(Metro {
        slug: slug.to_string(),
        name: f.name.unwrap_or_else(|| slug.to_string()),
        country: f.country.unwrap_or_default(),
        aliases: f.aliases,
    })
}

pub fn list_metros(vault_path: &str) -> Result<Vec<Metro>, String> {
    note::read_notes_in(&Path::new(vault_path).join("metros"), parse_metro)
}

/// Normalized key → set of metro slugs that own that key.
pub struct MetroIndex {
    keys: HashMap<String, HashSet<String>>,
}

impl MetroIndex {
    pub fn build(metros: &[Metro]) -> Self {
        let mut keys: HashMap<String, HashSet<String>> = HashMap::new();
        for m in metros {
            for raw in std::iter::once(m.slug.as_str())
                .chain(std::iter::once(m.name.as_str()))
                .chain(m.aliases.iter().map(String::as_str))
            {
                let k = slugify(raw);
                if !k.is_empty() {
                    keys.entry(k).or_default().insert(m.slug.clone());
                }
            }
        }
        MetroIndex { keys }
    }

    /// Return the metro slug for a free-text location, or `None` when the location
    /// is ambiguous, unrecognised, or empty.
    ///
    /// Algorithm:
    /// 1. Build candidates: the whole string, then each comma-split part.
    /// 2. For each candidate, slugify and look up in the index:
    ///    - exactly one slug → hit;
    ///    - >1 slugs (ambiguous key) → skip (a more-specific part may still resolve);
    ///    - no entry → skip.
    /// 3. Collect distinct hit slugs. Return `Some(slug)` iff exactly one distinct slug
    ///    was collected; `None` for zero hits or conflicting hits.
    pub fn resolve(&self, location: &str) -> Option<String> {
        let candidates: Vec<&str> = std::iter::once(location)
            .chain(location.split(','))
            .collect();

        let mut hits: HashSet<String> = HashSet::new();
        for candidate in candidates {
            let k = slugify(candidate);
            if k.is_empty() {
                continue;
            }
            if let Some(slugs) = self.keys.get(&k) {
                if slugs.len() == 1 {
                    hits.insert(slugs.iter().next().unwrap().clone());
                }
                // >1 slugs → ambiguous key, skip
            }
        }

        if hits.len() == 1 {
            hits.into_iter().next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx() -> MetroIndex {
        MetroIndex::build(&[
            Metro {
                slug: "virginia-beach-norfolk-newport-news-va-nc".into(),
                name: "Virginia Beach-Norfolk-Newport News, VA-NC".into(),
                country: "US".into(),
                aliases: vec![
                    "Virginia Beach".into(),
                    "Norfolk".into(),
                    "Newport News".into(),
                    "Hampton Roads".into(),
                ],
            },
            Metro {
                slug: "richmond-va".into(),
                name: "Richmond, VA".into(),
                country: "US".into(),
                aliases: vec!["Richmond".into()],
            },
        ])
    }

    #[test]
    fn resolve_alias_direct() {
        assert_eq!(
            idx().resolve("Norfolk"),
            Some("virginia-beach-norfolk-newport-news-va-nc".into())
        );
    }

    #[test]
    fn resolve_alias_with_comma_suffix() {
        assert_eq!(
            idx().resolve("Norfolk, VA"),
            Some("virginia-beach-norfolk-newport-news-va-nc".into())
        );
    }

    #[test]
    fn resolve_multiword_alias() {
        assert_eq!(
            idx().resolve("Hampton Roads"),
            Some("virginia-beach-norfolk-newport-news-va-nc".into())
        );
    }

    #[test]
    fn resolve_case_insensitive() {
        assert_eq!(idx().resolve("richmond"), Some("richmond-va".into()));
    }

    #[test]
    fn resolve_unknown_returns_none() {
        assert_eq!(idx().resolve("Remote"), None);
        assert_eq!(idx().resolve("Atlantis"), None);
    }

    #[test]
    fn resolve_empty_returns_none() {
        assert_eq!(idx().resolve(""), None);
    }

    #[test]
    fn list_metros_missing_dir_returns_empty() {
        let result = list_metros("/no/such/lodestar/vault/path");
        assert!(result.unwrap().is_empty());
    }
}
