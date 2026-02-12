use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use chrono::Utc;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub(crate) fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create parent directories {}",
                parent.to_string_lossy()
            )
        })?;
    }
    Ok(())
}

pub(crate) fn create_hard_link_checked(source: &Path, target: &Path) -> Result<()> {
    let source_meta = fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect source {}", source.display()))?;

    if !source_meta.file_type().is_file() {
        return Err(anyhow!(
            "source is not a regular file: {}",
            source.display()
        ));
    }

    check_same_filesystem(&source_meta, target)?;

    fs::hard_link(source, target).with_context(|| {
        format!(
            "failed to create hardlink {} -> {}",
            target.display(),
            source.display()
        )
    })?;

    Ok(())
}

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct BackupOutcome {
    pub(crate) backup_path: Option<PathBuf>,
}

impl BackupOutcome {
    pub(crate) const fn none() -> Self {
        Self {
            backup_path: None,
        }
    }
}

pub(crate) fn remove_existing_target_file(
    target: &Path,
    backup_dir: Option<&Path>,
) -> Result<BackupOutcome> {
    match fs::symlink_metadata(target) {
        Ok(meta) => {
            if meta.is_dir() {
                return Err(anyhow!(
                    "target is a directory; refusing to replace: {}",
                    target.display()
                ));
            }

            if let Some(backup_root) = backup_dir {
                return backup_target_file(target, backup_root, meta.len());
            }

            fs::remove_file(target).with_context(|| {
                format!("failed to remove existing target {}", target.display())
            })?;
            Ok(BackupOutcome::none())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BackupOutcome::none()),
        Err(err) => Err(anyhow!(
            "failed to inspect existing target {}: {}",
            target.display(),
            err
        )),
    }
}

fn backup_target_file(target: &Path, backup_root: &Path, file_size: u64) -> Result<BackupOutcome> {
    check_disk_space(backup_root, file_size)?;

    fs::create_dir_all(backup_root).with_context(|| {
        format!("failed to create backup directory {}", backup_root.display())
    })?;

    let backup_path = build_backup_path(backup_root, target);

    match fs::rename(target, &backup_path) {
        Ok(_) => finalize_backup(backup_root, backup_path, file_size),
        Err(_) => {
            fs::copy(target, &backup_path).with_context(|| {
                format!("failed to copy target to backup {}", backup_path.display())
            })?;
            fs::remove_file(target).with_context(|| {
                format!("failed to remove existing target {}", target.display())
            })?;
            finalize_backup(backup_root, backup_path, file_size)
        }
    }
}

fn build_backup_path(backup_root: &Path, target: &Path) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target".to_owned());
    backup_root.join(format!("{}-{}", ts, file_name))
}

fn finalize_backup(backup_root: &Path, backup_path: PathBuf, file_size: u64) -> Result<BackupOutcome> {
    if let Ok(hash) = calculate_sha256(&backup_path) {
        let _ = save_hash_metadata(&backup_path, &hash, file_size);
    }
    let _ = cleanup_old_backups(backup_root, 100);

    Ok(BackupOutcome {
        backup_path: Some(backup_path),
    })
}

#[cfg(unix)]
fn check_same_filesystem(source_meta: &fs::Metadata, target: &Path) -> Result<()> {
    let target_parent = target.parent().unwrap_or_else(|| Path::new("."));
    let parent_meta = fs::metadata(target_parent).with_context(|| {
        format!(
            "failed to inspect target parent directory {}",
            target_parent.display()
        )
    })?;

    if source_meta.dev() != parent_meta.dev() {
        return Err(anyhow!(
            "hardlink across filesystems is not supported: source={} target_parent={}",
            source_meta.dev(),
            parent_meta.dev()
        ));
    }

    Ok(())
}

#[cfg(not(unix))]
fn check_same_filesystem(_source_meta: &fs::Metadata, _target: &Path) -> Result<()> {
    Ok(())
}

// Phase 1: SHA256 Hash calculation
pub(crate) fn calculate_sha256(path: &Path) -> Result<String> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open file for hashing {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut reader = std::io::BufReader::new(file);

    std::io::copy(&mut reader, &mut hasher)
        .with_context(|| format!("failed to read file for hashing {}", path.display()))?;

    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn save_hash_metadata(backup_path: &Path, hash: &str, file_size: u64) -> Result<()> {
    let hash_path = backup_path.with_extension(format!("{}.sha256", 
        backup_path.extension().map(|e| e.to_string_lossy()).unwrap_or_default()));
    
    let metadata = format!(
        "algorithm=sha256\nhash={}\nsize={}\ntimestamp={}\n",
        hash,
        file_size,
        Utc::now().to_rfc3339()
    );
    
    fs::write(&hash_path, metadata)
        .with_context(|| format!("failed to write hash metadata to {}", hash_path.display()))?;
    
    Ok(())
}

// Phase 2: Disk space check
#[cfg(unix)]
pub(crate) fn check_disk_space(path: &Path, required_bytes: u64) -> Result<()> {
    use std::ffi::CString;
    
    let target_parent = path.parent().unwrap_or_else(|| Path::new("."));
    let path_cstr = CString::new(target_parent.to_string_lossy().as_bytes())
        .map_err(|_| anyhow!("invalid path for disk space check"))?;
    
    let stat = unsafe {
        let mut stat_buf = std::mem::MaybeUninit::uninit();
        if libc::statfs(path_cstr.as_ptr(), stat_buf.as_mut_ptr()) != 0 {
            return Err(anyhow!("failed to check disk space for {}", target_parent.display()));
        }
        stat_buf.assume_init()
    };
    
    let available_bytes = (stat.f_bavail as u64) * (stat.f_bsize as u64);
    
    if available_bytes < required_bytes {
        return Err(anyhow!(
            "insufficient disk space: required={} bytes, available={} bytes",
            required_bytes,
            available_bytes
        ));
    }
    
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn check_disk_space(_path: &Path, _required_bytes: u64) -> Result<()> {
    // Simplified check on non-Unix platforms
    Ok(())
}

// Phase 3: Version limit management
pub(crate) fn cleanup_old_backups(backup_dir: &Path, max_versions: usize) -> Result<()> {
    if !backup_dir.exists() {
        return Ok(());
    }
    
    let mut backup_files = Vec::new();
    
    for entry in fs::read_dir(backup_dir)
        .with_context(|| format!("failed to read backup directory {}", backup_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        
        // Only process .bak files
        if path.extension().is_some_and(|ext| ext == "bak")
            && let Ok(meta) = fs::metadata(&path)
            && let Ok(modified) = meta.modified()
        {
            backup_files.push((path, modified));
        }
    }
    
    if backup_files.len() > max_versions {
        // Sort by modification time (oldest first)
        backup_files.sort_by(|a, b| a.1.cmp(&b.1));
        
        let to_remove = backup_files.len() - max_versions;
        for (path, _) in backup_files.iter().take(to_remove) {
            // Remove the backup file
            let _ = fs::remove_file(path);
            
            // Also remove associated .sha256 file if exists
            let sha256_path = path.with_extension(format!("{}.sha256", 
                path.extension().map(|e| e.to_string_lossy()).unwrap_or_default()));
            let _ = fs::remove_file(sha256_path);
        }
    }
    
    Ok(())
}
