//! Alias-aware competency index: maps a JD's free-text skill term to one of the user's
//! competency notes (name / alias / slug), case-insensitively via `note::slugify` normalization.
#![allow(dead_code)]

use crate::note::{self, slugify, split_frontmatter};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

pub struct Competency {
    pub slug: String,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Deserialize)]
struct Front {
    name: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

fn parse_competency(slug: &str, text: &str) -> Result<Competency, String> {
    let (fm, _) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    Ok(Competency {
        slug: slug.to_string(),
        name: f.name.unwrap_or_else(|| slug.to_string()),
        aliases: f.aliases,
    })
}

pub fn list_competencies(vault_path: &str) -> Result<Vec<Competency>, String> {
    note::read_notes_in(
        &Path::new(vault_path).join("competencies"),
        parse_competency,
    )
}

pub struct CompetencyIndex {
    keys: HashSet<String>,
}

impl CompetencyIndex {
    pub fn build(comps: &[Competency]) -> Self {
        let mut keys = HashSet::new();
        for c in comps {
            keys.insert(slugify(&c.slug));
            keys.insert(slugify(&c.name));
            for a in &c.aliases {
                keys.insert(slugify(a));
            }
        }
        keys.remove(""); // slugify of punctuation-only yields ""
        CompetencyIndex { keys }
    }

    pub fn matches(&self, skill: &str) -> bool {
        let k = slugify(skill);
        !k.is_empty() && self.keys.contains(&k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_matches_name_alias_and_slug_case_insensitively() {
        let comps = vec![
            Competency {
                slug: "accessibility".into(),
                name: "Accessibility".into(),
                aliases: vec!["a11y".into(), "WCAG".into()],
            },
            Competency {
                slug: "ruby-on-rails".into(),
                name: "Ruby on Rails".into(),
                aliases: vec!["Rails".into(), "RoR".into()],
            },
        ];
        let idx = CompetencyIndex::build(&comps);
        assert!(idx.matches("a11y")); // alias
        assert!(idx.matches("Accessibility")); // name
        assert!(idx.matches("accessibility")); // slug / case-insensitive
        assert!(idx.matches("Ruby on Rails")); // multi-word name
        assert!(idx.matches("rails")); // alias case-insensitive
        assert!(!idx.matches("Cobol")); // unknown
    }
}
