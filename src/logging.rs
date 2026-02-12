use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const LOG_FILE_NAME: &str = ".operations.log";
const LOG_SIZE_LIMIT: u64 = 1024 * 1024; // 1MB

#[derive(Debug, Clone)]
pub(crate) enum Action {
    Replace,
    #[allow(dead_code)]
    Backup,
}

impl Action {
    fn as_str(&self) -> &str {
        match self {
            Action::Replace => "replace",
            Action::Backup => "backup",
        }
    }
}

pub(crate) struct OperationLog {
    log_path: std::path::PathBuf,
}

pub(crate) struct LogEntry<'a> {
    pub action: Action,
    pub source: &'a Path,
    pub target: &'a Path,
    pub status: &'a str,
    pub error: Option<&'a str>,
    pub hash_before: Option<&'a str>,
    pub backup_location: Option<&'a Path>,
}

impl OperationLog {
    pub(crate) fn new(backup_dir: &Path) -> Self {
        let log_path = backup_dir.join(LOG_FILE_NAME);
        OperationLog { log_path }
    }

    pub(crate) fn record(&self, entry_data: LogEntry<'_>) -> Result<()> {
        let entry = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "action": entry_data.action.as_str(),
            "source": entry_data.source.to_string_lossy(),
            "target": entry_data.target.to_string_lossy(),
            "status": entry_data.status,
            "error": entry_data.error,
            "hash_before": entry_data.hash_before,
            "backup_location": entry_data.backup_location.map(|p| p.to_string_lossy())
        });

        // Check if we need to rotate the log
        if let Ok(meta) = fs::metadata(&self.log_path)
            && meta.len() > LOG_SIZE_LIMIT
        {
            self.rotate_log()?;
        }

        let log_contents = if self.log_path.exists() {
            fs::read_to_string(&self.log_path).unwrap_or_else(|_| String::from("[]"))
        } else {
            String::from("[]")
        };

        // Parse as JSON array
        let mut entries: Vec<Value> =
            serde_json::from_str(&log_contents).unwrap_or_else(|_| Vec::new());

        entries.push(entry);

        // Write back
        let json_str =
            serde_json::to_string_pretty(&entries).context("failed to serialize log entries")?;

        fs::write(&self.log_path, json_str)
            .with_context(|| format!("failed to write log to {}", self.log_path.display()))?;

        Ok(())
    }

    fn rotate_log(&self) -> Result<()> {
        let rotated_path = self.log_path.with_extension("log.1");

        // If rotated file exists, remove it
        if rotated_path.exists() {
            fs::remove_file(&rotated_path)?;
        }

        // Rename current log to .1
        if self.log_path.exists() {
            fs::rename(&self.log_path, &rotated_path)?;
        }

        Ok(())
    }
}
