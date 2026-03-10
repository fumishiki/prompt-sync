use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::Profile;
use crate::model::ResolveContext;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ConfigFile {
    #[serde(default)]
    pub(crate) master: Option<MasterConfig>,
    #[serde(default)]
    pub(crate) links: Vec<LinkRule>,
    #[serde(default)]
    pub(crate) skills_sets: Vec<SkillsSet>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct MasterConfig {
    #[serde(default)]
    pub(crate) root: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct LinkRule {
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) targets: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct SkillsSet {
    pub(crate) source_root: String,
    #[serde(default)]
    pub(crate) target_roots: Vec<String>,
    #[serde(default)]
    pub(crate) exclude: Vec<String>,
    #[serde(default)]
    pub(crate) only_skills: Vec<String>,
    #[serde(default)]
    pub(crate) exclude_skills: Vec<String>,
}

pub(crate) fn load_config(config_path: &Path) -> Result<(ConfigFile, ResolveContext)> {
    let config_text = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read config: {}", config_path.display()))?;
    let config: ConfigFile = toml::from_str(&config_text)
        .with_context(|| format!("invalid TOML config: {}", config_path.display()))?;
    let ctx = build_resolve_context(config_path)?;

    Ok((config, ctx))
}

/// Profile → link target path mapping (order defines output order).
const LINK_TARGETS: &[(Profile, &str)] = &[
    (Profile::Codex, "~/.codex/AGENTS.md"),
    (Profile::Claude, "~/.claude/CLAUDE.md"),
    (Profile::Gemini, "~/.gemini/GEMINI.md"),
    (Profile::Copilot, "<repo>/.github/copilot-instructions.md"),
    (Profile::Kiro, "~/.kiro/steering/master.md"),
];

/// Profile → skills target root mapping (order defines output order).
const SKILL_TARGET_ROOTS: &[(Profile, &str)] = &[
    (Profile::Claude, "~/.claude/skills"),
    (Profile::Gemini, "~/.gemini/skills"),
    (Profile::Codex, "~/.codex/skills"),
    (Profile::Kiro, "~/.kiro/steering"),
];

/// Legacy skills targets that receive a copy from the Codex skills directory.
const LEGACY_SKILL_TARGET_ROOTS: &[(Profile, &str)] = &[
    (Profile::Claude, "~/.claude/skills"),
];

pub(crate) fn build_default_config(profiles: &[Profile]) -> ConfigFile {
    let profile_set: HashSet<_> = profiles.iter().copied().collect();

    let profile_targets = |table: &[(Profile, &str)]| -> Vec<String> {
        table
            .iter()
            .filter(|(p, _)| profile_set.contains(p))
            .map(|(_, t)| (*t).to_owned())
            .collect()
    };

    let link_targets = profile_targets(LINK_TARGETS);
    let target_roots = profile_targets(SKILL_TARGET_ROOTS);
    let legacy_targets = profile_targets(LEGACY_SKILL_TARGET_ROOTS);

    let mut skills_sets = Vec::new();
    if !target_roots.is_empty() {
        skills_sets.push(SkillsSet {
            source_root: "~/.agents/skills".to_owned(),
            target_roots,
            ..Default::default()
        });
    }
    if !legacy_targets.is_empty() {
        skills_sets.push(SkillsSet {
            source_root: "~/.codex/skills".to_owned(),
            target_roots: legacy_targets,
            exclude: vec!["*/.system/**".to_owned()],
            ..Default::default()
        });
    }

    ConfigFile {
        master: Some(MasterConfig {
            root: Some("~/.ai_settings".to_owned()),
        }),
        links: vec![LinkRule {
            source: "~/.ai_settings/master.md".to_owned(),
            targets: link_targets,
        }],
        skills_sets,
    }
}

pub(crate) fn build_resolve_context(config_path: &Path) -> Result<ResolveContext> {
    let config_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let repo_root = env::current_dir().context("failed to resolve current directory")?;
    let home_dir = env::var_os("HOME").map(PathBuf::from);
    let repo_root_text = repo_root.to_string_lossy().into_owned();
    let home_dir_text = home_dir
        .as_ref()
        .map(|dir| dir.to_string_lossy().into_owned());

    Ok(ResolveContext {
        config_dir,
        repo_root_text,
        home_dir,
        home_dir_text,
    })
}

pub(crate) fn build_bootstrap_config() -> ConfigFile {
    ConfigFile {
        master: Some(MasterConfig {
            root: Some("~/.ai_settings".to_owned()),
        }),
        links: vec![LinkRule {
            source: "~/.ai_settings/master.md".to_owned(),
            targets: [
                "~/.codex/AGENTS.md",
                "~/.claude/CLAUDE.md",
                "~/.gemini/GEMINI.md",
                "<repo>/AGENTS.md",
                "<repo>/CLAUDE.md",
                "<repo>/GEMINI.md",
                "<repo>/.github/copilot-instructions.md",
                "~/.kiro/steering/master.md",
            ]
            .map(String::from)
            .to_vec(),
        }],
        skills_sets: vec![
            SkillsSet {
                source_root: "~/.agents/skills".to_owned(),
                target_roots: [
                    "~/.claude/skills",
                    "~/.gemini/skills",
                    "~/.codex/skills",
                    "<repo>/.claude/skills",
                    "<repo>/.gemini/skills",
                    "<repo>/.agents/skills",
                    "~/.kiro/steering",
                ]
                .map(String::from)
                .to_vec(),
                ..Default::default()
            },
            SkillsSet {
                source_root: "~/.codex/skills".to_owned(),
                target_roots: ["~/.claude/skills"].map(String::from).to_vec(),
                exclude: ["*/.system/**"].map(String::from).to_vec(),
                ..Default::default()
            },
        ],
    }
}
