//! Community involvement entity: parses community notes from `profile/community/*.md`.
//! Mirrors the `experience` reader pattern — `read_notes_in`, frontmatter + body.
#![allow(dead_code)]

use crate::note::{self, split_frontmatter};
use serde::Deserialize;
use std::path::Path;

pub struct Community {
    pub slug: String,
    pub organization: String,
    pub role: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub relevance_tags: Vec<String>,
    /// The note body (everything after the frontmatter), trimmed.
    pub body: String,
}

#[derive(Deserialize)]
struct Front {
    organization: Option<String>,
    role: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    #[serde(default)]
    relevance_tags: Vec<String>,
}

fn parse_community(slug: &str, text: &str) -> Result<Community, String> {
    let (fm, body) = split_frontmatter(text);
    let f: Front = serde_yaml::from_str(fm).map_err(|e| format!("{slug}: {e}"))?;
    Ok(Community {
        slug: slug.to_string(),
        organization: f.organization.unwrap_or_default(),
        role: f.role.unwrap_or_default(),
        start_date: f.start_date.filter(|s| !s.trim().is_empty()),
        end_date: f.end_date.filter(|s| !s.trim().is_empty()),
        relevance_tags: f.relevance_tags,
        body: body.trim().to_string(),
    })
}

pub fn list_community(vault_path: &str) -> Result<Vec<Community>, String> {
    note::read_notes_in(
        &Path::new(vault_path).join("profile").join("community"),
        parse_community,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_community_round_trips_fixture_note() {
        let dir = std::env::temp_dir().join(format!("lodestar-comm-{}", std::process::id()));
        let comm = dir.join("profile").join("community");
        std::fs::create_dir_all(&comm).unwrap();
        std::fs::write(
            comm.join("757colorcoded.md"),
            "---\nid: 757colorcoded\ntype: community\norganization: 757ColorCoded\nrole: Web Dev Team Lead\nstart_date: 2018-08\nend_date: 2023-08\nrelevance_tags: [technical_education, community_building, leadership]\n---\nHampton Roads community for people of color in tech.\n",
        )
        .unwrap();
        let got = list_community(dir.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(got.len(), 1);
        let c = &got[0];
        assert_eq!(c.slug, "757colorcoded");
        assert_eq!(c.organization, "757ColorCoded");
        assert_eq!(c.role, "Web Dev Team Lead");
        assert_eq!(c.start_date.as_deref(), Some("2018-08"));
        assert_eq!(c.end_date.as_deref(), Some("2023-08"));
        assert_eq!(
            c.relevance_tags,
            vec!["technical_education".to_string(), "community_building".to_string(), "leadership".to_string()]
        );
        assert!(c.body.contains("Hampton Roads community"), "body must be captured");
    }

    #[test]
    fn list_community_skips_template_notes() {
        let dir = std::env::temp_dir().join(format!("lodestar-comm-tpl-{}", std::process::id()));
        let comm = dir.join("profile").join("community");
        std::fs::create_dir_all(&comm).unwrap();
        std::fs::write(
            comm.join("_template.md"),
            "---\norganization: Template Org\nrole: Role\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(
            comm.join("real-org.md"),
            "---\norganization: Real Org\nrole: Volunteer\nrelevance_tags: [mentorship]\n---\nSome body.\n",
        )
        .unwrap();
        let got = list_community(dir.to_str().unwrap()).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(got.len(), 1, "_template must be skipped; got {} entries", got.len());
        assert_eq!(got[0].organization, "Real Org");
    }

    #[test]
    fn list_community_missing_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!("lodestar-comm-none-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert!(list_community(dir.to_str().unwrap()).unwrap().is_empty());
    }
}
