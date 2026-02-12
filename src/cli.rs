use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "prompt-sync",
    version,
    about = "Hardlink manager for AI instruction/skills files"
)]
pub struct Cli {
    /// Path to config TOML.
    #[arg(long, default_value = "prompt-sync.toml")]
    pub config: PathBuf,

    /// Verbose output.
    #[arg(long, short)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Generate initial config file.
    Init {
        /// Overwrite existing config.
        #[arg(long)]
        force: bool,

        /// Include vendor profile(s) in the generated template.
        #[arg(long = "profile", value_enum)]
        profiles: Vec<Profile>,
    },
    /// Create/update hardlinks based on config.
    Link {
        /// Only create links that do not exist yet.
        #[arg(long)]
        only_missing: bool,

        /// Replace existing conflicting targets.
        #[arg(long)]
        force: bool,

        /// Show planned changes without touching files.
        #[arg(long)]
        dry_run: bool,

        /// Emit JSON output.
        #[arg(long)]
        json: bool,

        /// Backup directory for files replaced by --force.
        #[arg(long)]
        backup_dir: Option<PathBuf>,
    },
    /// Verify link integrity.
    Verify {
        /// Emit JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Repair missing/broken links.
    Repair {
        /// Also overwrite CONFLICT targets.
        #[arg(long)]
        force: bool,

        /// Show planned changes without touching files.
        #[arg(long)]
        dry_run: bool,

        /// Emit JSON output.
        #[arg(long)]
        json: bool,

        /// Backup directory for files replaced by --force.
        #[arg(long)]
        backup_dir: Option<PathBuf>,
    },
    /// Print short status summary.
    Status {
        /// Emit JSON output.
        #[arg(long)]
        json: bool,
    },
    /// One-tap setup for common vendor paths (alias: magic).
    #[command(visible_alias = "magic")]
    Bootstrap {
        /// Replace existing conflicting targets.
        #[arg(long)]
        force: bool,

        /// Show planned changes without touching files.
        #[arg(long)]
        dry_run: bool,

        /// Emit JSON output.
        #[arg(long)]
        json: bool,

        /// Persist discovered config into --config path.
        #[arg(long)]
        write_config: bool,

        /// Backup directory for files replaced by --force.
        #[arg(long)]
        backup_dir: Option<PathBuf>,
    },
    /// Install commit-msg hook to block AI co-author trailers.
    InstallCommitGuard {
        /// Repository root path. Defaults to current directory.
        #[arg(long, default_value = ".")]
        repo: PathBuf,

        /// Overwrite existing hook file.
        #[arg(long)]
        force: bool,

        /// Show planned changes without touching files.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Codex,
    Claude,
    Gemini,
    Copilot,
}
