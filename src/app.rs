use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::cli::{Cli, Command, Profile};
use crate::config::{
    ConfigFile, build_bootstrap_config, build_default_config, build_resolve_context, load_config,
};
use crate::engine::{apply_link, apply_repair, build_mappings, inspect_mapping, print_report};
use crate::model::{Report, ResolveContext, Summary};
use crate::pathing::{absolute_path, resolve_path};
use crate::vcs::install_commit_guard;

pub(crate) fn run(cli: Cli) -> Result<i32> {
    let config_path = absolute_path(&cli.config)?;

    match cli.command {
        Command::Init { force, profiles } => run_init(&config_path, force, profiles),
        Command::Link {
            only_missing,
            force,
            dry_run,
            json,
            backup_dir,
        } => {
            let (config, ctx) = load_config(&config_path)?;
            let backup_dir = resolve_backup_dir(backup_dir.as_deref())?;
            let mappings = build_mappings(&config, &ctx, cli.verbose)?;
            let records = mappings
                .iter()
                .map(|mapping| apply_link(mapping, force, only_missing, dry_run, backup_dir.as_deref()))
                .collect::<Vec<_>>();
            let report = Report {
                command: "link".to_owned(),
                summary: Summary::from_records(&records),
                records,
            };
            print_report(&report, json, cli.verbose)?;
            Ok(exit_code(&report.summary, false))
        }
        Command::Verify { json } => {
            let (config, ctx) = load_config(&config_path)?;
            let mappings = build_mappings(&config, &ctx, cli.verbose)?;
            let records = mappings.iter().map(inspect_mapping).collect::<Vec<_>>();
            let report = Report {
                command: "verify".to_owned(),
                summary: Summary::from_records(&records),
                records,
            };
            print_report(&report, json, true)?;
            Ok(exit_code(&report.summary, true))
        }
        Command::Repair {
            force,
            dry_run,
            json,
            backup_dir,
        } => {
            let (config, ctx) = load_config(&config_path)?;
            let backup_dir = resolve_backup_dir(backup_dir.as_deref())?;
            let mappings = build_mappings(&config, &ctx, cli.verbose)?;
            let records = mappings
                .iter()
                .map(|mapping| apply_repair(mapping, force, dry_run, backup_dir.as_deref()))
                .collect::<Vec<_>>();
            let report = Report {
                command: "repair".to_owned(),
                summary: Summary::from_records(&records),
                records,
            };
            print_report(&report, json, cli.verbose)?;
            Ok(exit_code(&report.summary, true))
        }
        Command::Status { json } => {
            let (config, ctx) = load_config(&config_path)?;
            let mappings = build_mappings(&config, &ctx, cli.verbose)?;
            let records = mappings.iter().map(inspect_mapping).collect::<Vec<_>>();
            let report = Report {
                command: "status".to_owned(),
                summary: Summary::from_records(&records),
                records,
            };
            print_report(&report, json, false)?;
            Ok(exit_code(&report.summary, true))
        }
        Command::Bootstrap {
            force,
            dry_run,
            json,
            write_config,
            backup_dir,
        } => run_bootstrap(
            &config_path,
            force,
            dry_run,
            json,
            write_config,
            backup_dir.as_deref(),
            cli.verbose,
        ),
        Command::InstallCommitGuard {
            repo,
            force,
            dry_run,
        } => run_install_commit_guard(&repo, force, dry_run),
    }
}

fn run_init(config_path: &Path, force: bool, profiles: Vec<Profile>) -> Result<i32> {
    if config_path.exists() && !force {
        return Err(anyhow!(
            "config already exists: {} (use --force to overwrite)",
            config_path.display()
        ));
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create config directory: {}",
                parent.to_string_lossy()
            )
        })?;
    }

    let selected_profiles = if profiles.is_empty() {
        vec![
            Profile::Codex,
            Profile::Claude,
            Profile::Gemini,
            Profile::Copilot,
        ]
    } else {
        profiles
    };

    let config = build_default_config(&selected_profiles);
    let toml_text = toml::to_string_pretty(&config).context("failed to serialize config")?;

    fs::write(config_path, toml_text).with_context(|| {
        format!(
            "failed to write config file: {}",
            config_path.to_string_lossy()
        )
    })?;

    println!("created config: {}", config_path.display());
    Ok(0)
}

fn run_bootstrap(
    config_path: &Path,
    force: bool,
    dry_run: bool,
    json: bool,
    write_config: bool,
    backup_dir: Option<&Path>,
    verbose: bool,
) -> Result<i32> {
    let config = build_bootstrap_config();
    let ctx = build_resolve_context(config_path)?;

    if write_config {
        if config_path.exists() && !force {
            return Err(anyhow!(
                "config already exists: {} (use --force to overwrite)",
                config_path.display()
            ));
        }
        let text = toml::to_string_pretty(&config).context("failed to serialize config")?;
        if !dry_run {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "failed to create config directory: {}",
                        parent.to_string_lossy()
                    )
                })?;
            }
            fs::write(config_path, text).with_context(|| {
                format!(
                    "failed to write config file: {}",
                    config_path.to_string_lossy()
                )
            })?;
        }
        if verbose {
            eprintln!("bootstrap config prepared at: {}", config_path.display());
        }
    }

    prepare_bootstrap_sources(&config, &ctx, dry_run, verbose)?;
    let backup_dir = resolve_backup_dir(backup_dir)?;
    let mappings = build_mappings(&config, &ctx, verbose)?;
    let records = mappings
        .iter()
        .map(|mapping| apply_link(mapping, force, false, dry_run, backup_dir.as_deref()))
        .collect::<Vec<_>>();
    let report = Report {
        command: "bootstrap".to_owned(),
        summary: Summary::from_records(&records),
        records,
    };
    print_report(&report, json, verbose)?;
    Ok(exit_code(&report.summary, false))
}

fn resolve_backup_dir(backup_dir: Option<&Path>) -> Result<Option<std::path::PathBuf>> {
    backup_dir.map(absolute_path).transpose()
}

fn prepare_bootstrap_sources(
    config: &ConfigFile,
    ctx: &ResolveContext,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    for rule in &config.links {
        let source = resolve_path(&rule.source, ctx);
        if source.exists() {
            let meta = fs::symlink_metadata(&source)
                .with_context(|| format!("failed to inspect source file: {}", source.display()))?;
            if !meta.file_type().is_file() {
                return Err(anyhow!(
                    "bootstrap source must be a regular file: {}",
                    source.display()
                ));
            }
            continue;
        }
        if dry_run {
            if verbose {
                eprintln!(
                    "bootstrap dry-run: would create source file {}",
                    source.display()
                );
            }
            continue;
        }
        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create source parent directory: {}",
                    parent.display()
                )
            })?;
        }
        fs::write(
            &source,
            "# master instructions\n\nUpdate this file to sync all linked instruction files.\n",
        )
        .with_context(|| format!("failed to create source file: {}", source.display()))?;
        if verbose {
            eprintln!("bootstrap: created source file {}", source.display());
        }
    }

    for set in &config.skills_sets {
        let source_root = resolve_path(&set.source_root, ctx);
        if source_root.exists() {
            if !source_root.is_dir() {
                return Err(anyhow!(
                    "bootstrap skills source root must be a directory: {}",
                    source_root.display()
                ));
            }
            continue;
        }
        if dry_run {
            if verbose {
                eprintln!(
                    "bootstrap dry-run: would create skills source root {}",
                    source_root.display()
                );
            }
            continue;
        }
        fs::create_dir_all(&source_root).with_context(|| {
            format!(
                "failed to create skills source root directory: {}",
                source_root.display()
            )
        })?;
        if verbose {
            eprintln!(
                "bootstrap: created skills source root {}",
                source_root.display()
            );
        }
    }

    Ok(())
}

fn exit_code(summary: &Summary, include_inconsistency: bool) -> i32 {
    if summary.has_error() {
        2
    } else if include_inconsistency && summary.has_inconsistency() {
        1
    } else {
        0
    }
}

fn run_install_commit_guard(repo: &Path, force: bool, dry_run: bool) -> Result<i32> {
    let repo_root = absolute_path(repo)?;
    let hook_path = install_commit_guard(&repo_root, force, dry_run)?;
    if dry_run {
        println!("would install commit guard hook: {}", hook_path.display());
    } else {
        println!("installed commit guard hook: {}", hook_path.display());
    }
    Ok(0)
}
