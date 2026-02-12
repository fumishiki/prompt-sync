use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use walkdir::WalkDir;

use crate::config::ConfigFile;
use crate::logging::{self, OperationLog, Action};
use crate::model::{Mapping, MappingKind, Record, Report, ResolveContext, Status};
use crate::pathing::{hardlink_count, resolve_path, same_file};
use crate::safe_fs::{
    calculate_sha256,
    create_hard_link_checked,
    ensure_parent_dir,
    remove_existing_target_file,
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
        Status::Ok => Record {
            status: Status::Skipped,
            message: Some("already linked".to_owned()),
            ..current
        },
        Status::Missing => link_create(mapping, dry_run),
        Status::Broken | Status::Conflict => {
            if only_missing {
                return Record {
                    status: Status::Skipped,
                    message: Some("skipped by --only-missing".to_owned()),
                    ..current
                };
            }
            if !force {
                return Record {
                    status: Status::Error,
                    message: Some("target exists and differs (use --force)".to_owned()),
                    ..current
                };
            }
            link_replace(mapping, dry_run, backup_dir)
        }
        Status::Error => current,
        _ => Record {
            status: Status::Error,
            message: Some("unexpected state".to_owned()),
            ..current
        },
    }
}

pub(crate) fn apply_repair(mapping: &Mapping, force_conflict: bool, dry_run: bool, backup_dir: Option<&std::path::Path>) -> Record {
    let current = inspect_mapping(mapping);

    match current.status {
        Status::Ok => Record {
            status: Status::Skipped,
            message: Some("already healthy".to_owned()),
            ..current
        },
        Status::Missing => link_create(mapping, dry_run),
        Status::Broken => link_replace(mapping, dry_run, backup_dir),
        Status::Conflict => {
            if force_conflict {
                link_replace(mapping, dry_run, backup_dir)
            } else {
                Record {
                    status: Status::Skipped,
                    message: Some("conflict skipped (use --force to override)".to_owned()),
                    ..current
                }
            }
        }
        Status::Error => current,
        _ => Record {
            status: Status::Error,
            message: Some("unexpected state".to_owned()),
            ..current
        },
    }
}

pub(crate) fn inspect_mapping(mapping: &Mapping) -> Record {
    let base = base_record(mapping);

    let source_meta = match fs::symlink_metadata(&mapping.source) {
        Ok(meta) => meta,
        Err(err) => {
            return Record {
                status: Status::Error,
                message: Some(format!(
                    "source metadata error {}: {}",
                    mapping.source.display(),
                    err
                )),
                ..base
            };
        }
    };

    let target_meta = match fs::symlink_metadata(&mapping.target) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Record {
                status: Status::Missing,
                message: Some("target missing".to_owned()),
                ..base
            };
        }
        Err(err) => {
            return Record {
                status: Status::Error,
                message: Some(format!(
                    "target metadata error {}: {}",
                    mapping.target.display(),
                    err
                )),
                ..base
            };
        }
    };

    if !source_meta.file_type().is_file() {
        return Record {
            status: Status::Error,
            message: Some("source is not a regular file".to_owned()),
            ..base
        };
    }

    if !target_meta.file_type().is_file() {
        return Record {
            status: Status::Conflict,
            message: Some("target exists but is not a regular file".to_owned()),
            ..base
        };
    }

    if same_file(&source_meta, &target_meta) {
        return Record {
            status: Status::Ok,
            message: Some("inode match".to_owned()),
            ..base
        };
    }

    if hardlink_count(&target_meta) > 1 {
        return Record {
            status: Status::Broken,
            message: Some("target is hardlinked to a different source".to_owned()),
            ..base
        };
    }

    Record {
        status: Status::Conflict,
        message: Some("target differs and is not linked".to_owned()),
        ..base
    }
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
        return Record {
            status: Status::WouldCreate,
            message: Some("would create hardlink".to_owned()),
            ..base
        };
    }

    if let Err(err) = ensure_parent_dir(&mapping.target) {
        return Record {
            status: Status::Error,
            message: Some(err.to_string()),
            ..base
        };
    }

    if let Err(err) = create_hard_link_checked(&mapping.source, &mapping.target) {
        return Record {
            status: Status::Error,
            message: Some(err.to_string()),
            ..base
        };
    }

    Record {
        status: Status::Created,
        message: Some("created hardlink".to_owned()),
        ..base
    }
}

fn link_replace(mapping: &Mapping, dry_run: bool, backup_dir: Option<&std::path::Path>) -> Record {
    let base = base_record(mapping);

    if dry_run {
        return Record {
            status: Status::WouldReplace,
            message: Some("would replace target with hardlink".to_owned()),
            ..base
        };
    }

    if let Err(err) = ensure_parent_dir(&mapping.target) {
        if let Some(backup_root) = backup_dir {
            let logger = OperationLog::new(backup_root);
            let _ = logger.record(logging::LogEntry {
                action: Action::Replace,
                source: &mapping.source,
                target: &mapping.target,
                status: "failed",
                error: Some(&err.to_string()),
                hash_before: None,
                backup_location: None,
            });
        }
        return Record {
            status: Status::Error,
            message: Some(err.to_string()),
            ..base
        };
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
            if let Some(backup_root) = backup_dir {
                let logger = OperationLog::new(backup_root);
                let _ = logger.record(logging::LogEntry {
                    action: Action::Replace,
                    source: &mapping.source,
                    target: &mapping.target,
                    status: "failed",
                    error: Some(&err.to_string()),
                    hash_before: hash_before.as_deref(),
                    backup_location: None,
                });
            }
            return Record {
                status: Status::Error,
                message: Some(err.to_string()),
                ..base
            };
        }
    };

    if let Err(err) = create_hard_link_checked(&mapping.source, &mapping.target) {
        if let Some(backup_root) = backup_dir {
            let logger = OperationLog::new(backup_root);
            let _ = logger.record(logging::LogEntry {
                action: Action::Replace,
                source: &mapping.source,
                target: &mapping.target,
                status: "failed",
                error: Some(&err.to_string()),
                hash_before: hash_before.as_deref(),
                backup_location: backup_outcome.backup_path.as_deref(),
            });
        }
        return Record {
            status: Status::Error,
            message: Some(err.to_string()),
            ..base
        };
    }

    // Log successful replacement
    if let Some(backup_root) = backup_dir {
        let logger = OperationLog::new(backup_root);
        let _ = logger.record(logging::LogEntry {
            action: Action::Replace,
            source: &mapping.source,
            target: &mapping.target,
            status: "success",
            error: None,
            hash_before: hash_before.as_deref(),
            backup_location: backup_outcome.backup_path.as_deref(),
        });
    }

    Record {
        status: Status::Replaced,
        message: Some("replaced target with hardlink".to_owned()),
        ..base
    }
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
