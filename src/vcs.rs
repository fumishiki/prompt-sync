use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const COMMIT_GUARD_HOOK: &str = r#"#!/bin/sh
set -eu

msg_file="$1"
if [ ! -f "$msg_file" ]; then
  exit 0
fi

# Remove AI attribution lines automatically.
tmp_file="$(mktemp)"
grep -Eiv '^Co-authored-by:.*(chatgpt|claude|codex|gemini|copilot|openai|anthropic)' "$msg_file" \
  | grep -Eiv 'generated with.*(chatgpt|claude|codex|gemini|copilot|openai|anthropic)' \
  > "$tmp_file" || true
cat "$tmp_file" > "$msg_file"
rm -f "$tmp_file"

exit 0
"#;

pub(crate) fn install_commit_guard(
    repo_root: &Path,
    force: bool,
    dry_run: bool,
) -> Result<PathBuf> {
    let git_dir = resolve_git_dir(repo_root)?;
    let hook_path = git_dir.join("hooks").join("commit-msg");

    if hook_path.exists() && !force {
        return Err(anyhow!(
            "hook already exists: {} (use --force to overwrite)",
            hook_path.display()
        ));
    }

    if dry_run {
        return Ok(hook_path);
    }

    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create hook directory: {}",
                parent.to_string_lossy()
            )
        })?;
    }

    fs::write(&hook_path, COMMIT_GUARD_HOOK)
        .with_context(|| format!("failed to write hook: {}", hook_path.display()))?;

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&hook_path)
            .with_context(|| format!("failed to stat hook: {}", hook_path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&hook_path, permissions)
            .with_context(|| format!("failed to set executable bit: {}", hook_path.display()))?;
    }

    Ok(hook_path)
}

fn resolve_git_dir(repo_root: &Path) -> Result<PathBuf> {
    let dot_git = repo_root.join(".git");

    let meta = fs::symlink_metadata(&dot_git)
        .with_context(|| format!("failed to access .git in repo: {}", repo_root.display()))?;

    if meta.is_dir() {
        return Ok(dot_git);
    }

    if meta.is_file() {
        let raw = fs::read_to_string(&dot_git)
            .with_context(|| format!("failed to read .git file: {}", dot_git.display()))?;
        let line = raw.lines().next().unwrap_or_default().trim();
        const PREFIX: &str = "gitdir:";
        if let Some(rest) = line.strip_prefix(PREFIX) {
            let path = rest.trim();
            let resolved = if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                repo_root.join(path)
            };
            return Ok(resolved);
        }
        return Err(anyhow!("invalid .git file format: {}", dot_git.display()));
    }

    Err(anyhow!(
        ".git is neither directory nor file: {}",
        dot_git.display()
    ))
}
