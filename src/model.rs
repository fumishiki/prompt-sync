use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MappingKind {
    ConfigFile,
    SkillFile,
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    pub(crate) kind: MappingKind,
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ResolveContext {
    pub(crate) config_dir: PathBuf,
    pub(crate) repo_root_text: String,
    pub(crate) home_dir: Option<PathBuf>,
    pub(crate) home_dir_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum Status {
    Ok,
    Missing,
    Broken,
    Conflict,
    Created,
    Replaced,
    WouldCreate,
    WouldReplace,
    Skipped,
    Error,
}

#[derive(Debug, Serialize)]
pub(crate) struct Record {
    pub(crate) kind: MappingKind,
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
    pub(crate) status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) ok: usize,
    pub(crate) missing: usize,
    pub(crate) broken: usize,
    pub(crate) conflict: usize,
    pub(crate) created: usize,
    pub(crate) replaced: usize,
    pub(crate) would_create: usize,
    pub(crate) would_replace: usize,
    pub(crate) skipped: usize,
    pub(crate) errors: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub(crate) command: String,
    pub(crate) summary: Summary,
    pub(crate) records: Vec<Record>,
}

impl Summary {
    pub(crate) fn from_records(records: &[Record]) -> Self {
        let mut summary = Self {
            total: records.len(),
            ..Self::default()
        };

        for record in records {
            match record.status {
                Status::Ok => summary.ok += 1,
                Status::Missing => summary.missing += 1,
                Status::Broken => summary.broken += 1,
                Status::Conflict => summary.conflict += 1,
                Status::Created => summary.created += 1,
                Status::Replaced => summary.replaced += 1,
                Status::WouldCreate => summary.would_create += 1,
                Status::WouldReplace => summary.would_replace += 1,
                Status::Skipped => summary.skipped += 1,
                Status::Error => summary.errors += 1,
            }
        }

        summary
    }

    pub(crate) fn has_inconsistency(&self) -> bool {
        self.missing > 0 || self.broken > 0 || self.conflict > 0
    }

    pub(crate) fn has_error(&self) -> bool {
        self.errors > 0
    }
}
