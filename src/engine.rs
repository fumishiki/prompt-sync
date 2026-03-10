use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::config::ConfigFile;
use crate::logging::{self, Action, OperationLog};
use crate::model::{Mapping, MappingKind, Record, Report, ResolveContext, Status};
use crate::pathing::{hardlink_count, resolve_path, same_file};
use crate::safe_fs::{
    calculate_sha256, create_hard_link_checked, ensure_parent_dir, remove_existing_target_file,
};

pub(crate) fn build_mappings(
    config: &ConfigFile,
    ctx: &ResolveContext,
    verbose: bool,
) -> Result<Vec<Mapping>> {
    let mut mappings = Vec::new();
    let mut dedup: HashSet<(PathBuf, PathBuf)> = HashSet::new();

    for rule in &config.links {
        let source = resolve_path(&rule.source, ctx);
        for target_raw in &rule.targets {
            let target = resolve_path(target_raw, ctx);
            if dedup.insert((source.clone(), target.clone())) {
                mappings.push(Mapping {
                    kind: MappingKind::ConfigFile,
                    source: source.clone(),
                    target,
                });
            }
        }
    }

    for set in &config.skills_sets {
        let source_root = resolve_path(&set.source_root, ctx);
        if !source_root.exists() {
            if verbose {
                eprintln!(
                    "warn: source_root does not exist, skipped: {}",
                    source_root.display()
                );
            }
            continue;
        }
        if !source_root.is_dir() {
            return Err(anyhow!(
                "source_root is not a directory: {}",
                source_root.display()
            ));
        }

        let exclude_globs = build_glob_set(&set.exclude)?;

        for entry_result in WalkDir::new(&source_root) {
            let entry = entry_result.with_context(|| {
                format!("failed to walk source_root: {}", source_root.display())
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let source_file = entry.into_path();
            let rel = source_file.strip_prefix(&source_root).with_context(|| {
                format!(
                    "failed to compute relative path: {} in {}",
                    source_file.display(),
                    source_root.display()
                )
            })?;

            // Skill name filter (first path component = skill directory name)
            if let Some(skill_name) = extract_skill_name(rel) {
                if !set.only_skills.is_empty() {
                    if !set.only_skills.iter().any(|s| s == skill_name) {
                        continue;
                    }
                } else if !set.exclude_skills.is_empty()
                    && set.exclude_skills.iter().any(|s| s == skill_name)
                {
                    continue;
                }
            }

            // Exclude glob filter
            let rel_str = rel.to_string_lossy();
            if exclude_globs.is_match(rel_str.as_ref()) {
                continue;
            }

            for target_root_raw in &set.target_roots {
                let target_root = resolve_path(target_root_raw, ctx);
                let target = target_root.join(rel);
                if dedup.insert((source_file.clone(), target.clone())) {
                    mappings.push(Mapping {
                        kind: MappingKind::SkillFile,
                        source: source_file.clone(),
                        target,
                    });
                }
            }
        }
    }

    Ok(mappings)
}

pub(crate) fn apply_link(
    mapping: &Mapping,
    force: bool,
    only_missing: bool,
    dry_run: bool,
    backup_dir: Option<&std::path::Path>,
) -> Record {
    let current = inspect_mapping(mapping);

    match current.status {
        Status::Ok => current.with_status(Status::Skipped, "already linked"),
        Status::Missing => link_create(mapping, dry_run),
        Status::Broken | Status::Conflict => {
            if only_missing {
                return current.with_status(Status::Skipped, "skipped by --only-missing");
            }
            if !force {
                return current
                    .with_status(Status::Error, "target exists and differs (use --force)");
            }
            link_replace(mapping, dry_run, backup_dir)
        }
        Status::Error => current,
        _ => current.with_status(Status::Error, "unexpected state"),
    }
}

pub(crate) fn apply_repair(
    mapping: &Mapping,
    force_conflict: bool,
    dry_run: bool,
    backup_dir: Option<&std::path::Path>,
) -> Record {
    let current = inspect_mapping(mapping);

    match current.status {
        Status::Ok => current.with_status(Status::Skipped, "already healthy"),
        Status::Missing => link_create(mapping, dry_run),
        Status::Broken => link_replace(mapping, dry_run, backup_dir),
        Status::Conflict => {
            if force_conflict {
                link_replace(mapping, dry_run, backup_dir)
            } else {
                current.with_status(
                    Status::Skipped,
                    "conflict skipped (use --force to override)",
                )
            }
        }
        Status::Error => current,
        _ => current.with_status(Status::Error, "unexpected state"),
    }
}

pub(crate) fn inspect_mapping(mapping: &Mapping) -> Record {
    let base = base_record(mapping);

    let source_meta = match fs::symlink_metadata(&mapping.source) {
        Ok(meta) => meta,
        Err(err) => {
            return base.with_status(
                Status::Error,
                format!("source metadata error {}: {}", mapping.source.display(), err),
            );
        }
    };

    let target_meta = match fs::symlink_metadata(&mapping.target) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return base.with_status(Status::Missing, "target missing");
        }
        Err(err) => {
            return base.with_status(
                Status::Error,
                format!("target metadata error {}: {}", mapping.target.display(), err),
            );
        }
    };

    if !source_meta.file_type().is_file() {
        return base.with_status(Status::Error, "source is not a regular file");
    }

    if !target_meta.file_type().is_file() {
        return base.with_status(Status::Conflict, "target exists but is not a regular file");
    }

    if same_file(&source_meta, &target_meta) {
        return base.with_status(Status::Ok, "inode match");
    }

    if hardlink_count(&target_meta) > 1 {
        return base
            .with_status(Status::Broken, "target is hardlinked to a different source");
    }

    base.with_status(Status::Conflict, "target differs and is not linked")
}

pub(crate) fn print_report(report: &Report, json: bool, show_records_in_text: bool) -> Result<()> {
    if json {
        let json_text = serde_json::to_string_pretty(report).context("failed to serialize JSON")?;
        println!("{json_text}");
        return Ok(());
    }

    println!("command: {}", report.command);
    println!("total: {}", report.summary.total);
    println!(
        "ok={} missing={} broken={} conflict={} created={} replaced={} would_create={} would_replace={} skipped={} errors={}",
        report.summary.ok,
        report.summary.missing,
        report.summary.broken,
        report.summary.conflict,
        report.summary.created,
        report.summary.replaced,
        report.summary.would_create,
        report.summary.would_replace,
        report.summary.skipped,
        report.summary.errors,
    );

    if show_records_in_text {
        for record in &report.records {
            let message = record.message.as_deref().unwrap_or("");
            println!(
                "[{:?}] {} -> {} ({message})",
                record.status,
                record.source.display(),
                record.target.display(),
            );
        }
    } else {
        for record in report
            .records
            .iter()
            .filter(|record| record.status == Status::Error)
        {
            let message = record.message.as_deref().unwrap_or("");
            println!(
                "[{:?}] {} -> {} ({message})",
                record.status,
                record.source.display(),
                record.target.display(),
            );
        }
    }

    Ok(())
}

fn link_create(mapping: &Mapping, dry_run: bool) -> Record {
    let base = base_record(mapping);

    if dry_run {
        return base.with_status(Status::WouldCreate, "would create hardlink");
    }

    if let Err(err) = ensure_parent_dir(&mapping.target) {
        return base.with_status(Status::Error, err.to_string());
    }

    if let Err(err) = create_hard_link_checked(&mapping.source, &mapping.target) {
        return base.with_status(Status::Error, err.to_string());
    }

    base.with_status(Status::Created, "created hardlink")
}

fn link_replace(mapping: &Mapping, dry_run: bool, backup_dir: Option<&std::path::Path>) -> Record {
    let base = base_record(mapping);

    if dry_run {
        return base.with_status(Status::WouldReplace, "would replace target with hardlink");
    }

    if let Err(err) = ensure_parent_dir(&mapping.target) {
        log_replace(backup_dir, mapping, "failed", Some(&err.to_string()), None, None);
        return base.with_status(Status::Error, err.to_string());
    }

    // Calculate hash before replacement if backup is enabled
    let hash_before = if backup_dir.is_some() {
        calculate_sha256(&mapping.target).ok()
    } else {
        None
    };

    let backup_outcome = match remove_existing_target_file(&mapping.target, backup_dir) {
        Ok(outcome) => outcome,
        Err(err) => {
            log_replace(
                backup_dir,
                mapping,
                "failed",
                Some(&err.to_string()),
                hash_before.as_deref(),
                None,
            );
            return base.with_status(Status::Error, err.to_string());
        }
    };

    if let Err(err) = create_hard_link_checked(&mapping.source, &mapping.target) {
        log_replace(
            backup_dir,
            mapping,
            "failed",
            Some(&err.to_string()),
            hash_before.as_deref(),
            backup_outcome.backup_path.as_deref(),
        );
        return base.with_status(Status::Error, err.to_string());
    }

    log_replace(
        backup_dir,
        mapping,
        "success",
        None,
        hash_before.as_deref(),
        backup_outcome.backup_path.as_deref(),
    );
    base.with_status(Status::Replaced, "replaced target with hardlink")
}

fn log_replace(
    backup_dir: Option<&Path>,
    mapping: &Mapping,
    status: &str,
    error: Option<&str>,
    hash_before: Option<&str>,
    backup_location: Option<&Path>,
) {
    if let Some(backup_root) = backup_dir {
        let logger = OperationLog::new(backup_root);
        let _ = logger.record(logging::LogEntry {
            action: Action::Replace,
            source: &mapping.source,
            target: &mapping.target,
            status,
            error,
            hash_before,
            backup_location,
        });
    }
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)
            .with_context(|| format!("invalid exclude glob pattern: {pattern}"))?;
        builder.add(glob);
    }
    builder.build().context("failed to build glob set")
}

fn extract_skill_name(rel: &Path) -> Option<&str> {
    rel.components().next().and_then(|c| c.as_os_str().to_str())
}

fn base_record(mapping: &Mapping) -> Record {
    Record {
        kind: mapping.kind.clone(),
        source: mapping.source.clone(),
        target: mapping.target.clone(),
        status: Status::Error,
        message: None,
    }
}
