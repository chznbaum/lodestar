//! Capability-tier model map + schedule config, stored as JSON in the Tauri app config
//! dir (never the vault). Per design §4.5/§5.6: each LLM stage is classified by a tier
//! (`tier_for_stage`, in code); the tier→model map is the single user-editable knob.
//! Models are OpenRouter slugs — Anthropic-prioritized defaults, any model valid.
//!
//! Stage → tier classification is documented for users in `lodestar/docs/model-tiers.md`.
// `model_for`/`tier_for_stage`/`Tier`/`SPEED_MODEL` are consumed by the Phase A pipeline
// (not yet wired); suppress the dead-code lint until those callers exist.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Manager;

/// Default tier→model slugs (OpenRouter). Confirmed current 2026-06-18; these are just
/// the out-of-box defaults — the user edits the tier→model map in `config.json`.
pub const FRONTIER_MODEL: &str = "anthropic/claude-opus-4.8";
pub const BALANCED_MODEL: &str = "anthropic/claude-sonnet-4.6";
pub const SPEED_MODEL: &str = "anthropic/claude-haiku-4.5";

/// The capability tier a stage needs. Code-internal classification — NOT serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Frontier,
    Balanced,
    Speed,
}

/// The tier→model map: the single user-editable knob (the future settings surface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTiers {
    pub frontier: String,
    pub balanced: String,
    pub speed: String,
}

impl ModelTiers {
    pub fn model(&self, tier: Tier) -> &str {
        match tier {
            Tier::Frontier => &self.frontier,
            Tier::Balanced => &self.balanced,
            Tier::Speed => &self.speed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub tiers: ModelTiers,
    #[serde(default)]
    pub schedule_enabled: bool,
}

pub fn default_config() -> PipelineConfig {
    PipelineConfig {
        tiers: ModelTiers {
            frontier: FRONTIER_MODEL.to_string(),
            balanced: BALANCED_MODEL.to_string(),
            speed: SPEED_MODEL.to_string(),
        },
        schedule_enabled: false,
    }
}

/// The capability tier a pipeline stage needs (see `docs/model-tiers.md`). Unclassified
/// stages default to `Frontier` (quality-first): only proven objective-extraction stages
/// are explicitly downgraded.
pub fn tier_for_stage(stage: &str) -> Tier {
    match stage {
        "structure-listings" => Tier::Balanced,
        "structure-jd" | "research-gaps" | "alignment" => Tier::Frontier,
        _ => Tier::Frontier,
    }
}

pub fn parse_config(text: &str) -> Result<PipelineConfig, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

pub fn render_config(cfg: &PipelineConfig) -> String {
    serde_json::to_string_pretty(cfg).expect("config serializes") + "\n"
}

/// The model slug for a pipeline stage: its tier's configured model.
pub fn model_for(cfg: &PipelineConfig, stage: &str) -> String {
    cfg.tiers.model(tier_for_stage(stage)).to_string()
}

fn config_file(config_dir: &Path) -> std::path::PathBuf {
    config_dir.join("config.json")
}

/// Read the config; if absent or unreadable, return the default AND persist it (so the
/// file exists for the user to edit). A present-but-corrupt file falls back to default too.
pub fn load_config(config_dir: &Path) -> PipelineConfig {
    let path = config_file(config_dir);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = parse_config(&text) {
            return cfg;
        }
    }
    let cfg = default_config();
    let _ = save_config(config_dir, &cfg);
    cfg
}

pub fn save_config(config_dir: &Path, cfg: &PipelineConfig) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    std::fs::write(config_file(config_dir), render_config(cfg)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> Result<PipelineConfig, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(load_config(&dir))
}

#[tauri::command]
pub fn set_config(app: tauri::AppHandle, config: PipelineConfig) -> Result<PipelineConfig, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    save_config(&dir, &config)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_claude_tiers_and_schedule_off() {
        let c = default_config();
        assert_eq!(c.tiers.frontier, FRONTIER_MODEL);
        assert_eq!(c.tiers.balanced, BALANCED_MODEL);
        assert_eq!(c.tiers.speed, SPEED_MODEL);
        assert!(!c.schedule_enabled);
    }

    #[test]
    fn render_then_parse_round_trips() {
        let mut c = default_config();
        c.schedule_enabled = true;
        c.tiers.frontier = "anthropic/claude-opus-4.8-fast".into();
        let text = render_config(&c);
        assert_eq!(parse_config(&text).unwrap(), c);
    }

    #[test]
    fn stage_classification_maps_to_the_tier_model() {
        let c = default_config();
        // high-volume objective extraction → balanced
        assert_eq!(model_for(&c, "structure-listings"), BALANCED_MODEL);
        // nuanced extraction + reasoning steps → frontier
        assert_eq!(model_for(&c, "structure-jd"), FRONTIER_MODEL);
        assert_eq!(model_for(&c, "research-gaps"), FRONTIER_MODEL);
        assert_eq!(model_for(&c, "alignment"), FRONTIER_MODEL);
        // unclassified stage → frontier (quality-first default)
        assert_eq!(model_for(&c, "some-future-stage"), FRONTIER_MODEL);
    }

    #[test]
    fn model_for_tracks_config_edits_to_a_tier() {
        let mut c = default_config();
        c.tiers.balanced = "x/custom-worker".into();
        assert_eq!(model_for(&c, "structure-listings"), "x/custom-worker"); // balanced stage follows the edit
        assert_eq!(model_for(&c, "alignment"), FRONTIER_MODEL); // frontier stage unaffected
    }

    #[test]
    fn load_missing_returns_default_and_writes_it() {
        let dir = std::env::temp_dir().join(format!("lodestar-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = load_config(&dir); // absent -> default, persisted
        assert_eq!(c, default_config());
        assert!(dir.join("config.json").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
