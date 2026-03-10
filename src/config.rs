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

pub(crate) fn build_default_config(profiles: &[Profile]) -> ConfigFile {
    let profile_set: HashSet<_> = profiles.iter().copied().collect();
    let has = |p| profile_set.contains(&p);

    let link_targets: Vec<String> = [
        (Profile::Codex,   "~/.codex/AGENTS.md"),
        (Profile::Claude,  "~/.claude/CLAUDE.md"),
        (Profile::Gemini,  "~/.gemini/GEMINI.md"),
        (Profile::Copilot, "<repo>/.github/copilot-instructions.md"),
        (Profile::Kiro,    "~/.kiro/steering/master.md"),
    ]
    .into_iter()
    .filter(|(p, _)| has(*p))
    .map(|(_, t)| t.to_owned())
    .collect();

    let target_roots: Vec<String> = [
        (Profile::Claude, "~/.claude/skills"),
        (Profile::Gemini, "~/.gemini/skills"),
        (Profile::Codex,  "~/.codex/skills"),
        (Profile::Kiro,   "~/.kiro/steering"),
    ]
    .into_iter()
    .filter(|(p, _)| has(*p))
    .map(|(_, t)| t.to_owned())
    .collect();

    let mut skills_sets = Vec::new();
    if !target_roots.is_empty() {
        skills_sets.push(SkillsSet {
            source_root: "~/.agents/skills".to_owned(),
            target_roots,
            ..SkillsSet::default()
        });
    }
    if has(Profile::Claude) {
        skills_sets.push(SkillsSet {
            source_root: "~/.codex/skills".to_owned(),
            target_roots: vec!["~/.claude/skills".to_owned()],
            exclude: vec!["*/.system/**".to_owned()],
            ..SkillsSet::default()
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
            targets: vec![
                "~/.codex/AGENTS.md".to_owned(),
                "~/.claude/CLAUDE.md".to_owned(),
                "~/.gemini/GEMINI.md".to_owned(),
                "<repo>/AGENTS.md".to_owned(),
                "<repo>/CLAUDE.md".to_owned(),
                "<repo>/GEMINI.md".to_owned(),
                "<repo>/.github/copilot-instructions.md".to_owned(),
                "~/.kiro/steering/master.md".to_owned(),
            ],
        }],
        skills_sets: vec![
            SkillsSet {
                source_root: "~/.agents/skills".to_owned(),
                target_roots: vec![
                    "~/.claude/skills".to_owned(),
                    "~/.gemini/skills".to_owned(),
                    "~/.codex/skills".to_owned(),
                    "<repo>/.claude/skills".to_owned(),
                    "<repo>/.gemini/skills".to_owned(),
                    "<repo>/.agents/skills".to_owned(),
                    "~/.kiro/steering".to_owned(),
                ],
                exclude: Vec::new(),
                only_skills: Vec::new(),
                exclude_skills: Vec::new(),
            },
            SkillsSet {
                source_root: "~/.codex/skills".to_owned(),
                target_roots: vec!["~/.claude/skills".to_owned()],
                exclude: vec!["*/.system/**".to_owned()],
                only_skills: Vec::new(),
                exclude_skills: Vec::new(),
            },
        ],
    }
}
