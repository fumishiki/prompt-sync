use std::fs;
use std::path::Path;

use tempfile::TempDir;

use prompt_sync::{Cli, Command, run};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn link_then_verify_success() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    write_config(temp.path(), &source, &target)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);

    let verify_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Verify { json: false },
    })?;
    assert_eq!(verify_code, 0);

    #[cfg(unix)]
    {
        let source_meta = fs::metadata(&source)?;
        let target_meta = fs::metadata(&target)?;
        assert_eq!(source_meta.ino(), target_meta.ino());
        assert_eq!(source_meta.dev(), target_meta.dev());
    }

    Ok(())
}

#[test]
fn verify_missing_returns_one() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    write_config(temp.path(), &source, &target)?;

    let verify_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Verify { json: false },
    })?;
    assert_eq!(verify_code, 1);

    Ok(())
}

#[test]
fn link_conflict_without_force_returns_two() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing parent path"))?;
    fs::create_dir_all(parent)?;
    fs::write(&target, "local override")?;
    write_config(temp.path(), &source, &target)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 2);
    assert_eq!(fs::read_to_string(&target)?, "local override");

    Ok(())
}

#[test]
fn repair_conflict_with_force_replaces_target() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing parent path"))?;
    fs::create_dir_all(parent)?;
    fs::write(&target, "local override")?;
    write_config(temp.path(), &source, &target)?;

    let repair_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Repair {
            force: true,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(repair_code, 0);

    #[cfg(unix)]
    {
        let source_meta = fs::metadata(&source)?;
        let target_meta = fs::metadata(&target)?;
        assert_eq!(source_meta.ino(), target_meta.ino());
        assert_eq!(source_meta.dev(), target_meta.dev());
    }

    Ok(())
}

#[test]
fn link_dry_run_does_not_create_target() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    write_config(temp.path(), &source, &target)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: true,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);
    assert!(!target.exists());

    Ok(())
}

#[cfg(unix)]
#[test]
fn verify_symlink_target_is_conflict() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("master.md");
    let symlink_src = temp.path().join("other.md");
    let target = temp.path().join("out").join("AGENTS.md");

    fs::write(&source, "master instruction")?;
    fs::write(&symlink_src, "other instruction")?;
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing parent path"))?;
    fs::create_dir_all(parent)?;
    symlink(&symlink_src, &target)?;
    write_config(temp.path(), &source, &target)?;

    let verify_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Verify { json: false },
    })?;
    assert_eq!(verify_code, 1);

    Ok(())
}

#[test]
fn bootstrap_write_config_refuses_overwrite_without_force() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let config_path = temp.path().join("prompt-sync.toml");
    fs::write(&config_path, "# existing\n")?;

    let result = run(Cli {
        config: config_path.clone(),
        verbose: false,
        command: Command::Bootstrap {
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
            write_config: true,
        },
    });

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&config_path)?, "# existing\n");
    Ok(())
}

#[test]
fn install_commit_guard_creates_hook() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git").join("hooks"))?;

    let code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::InstallCommitGuard {
            repo: repo.clone(),
            force: false,
            dry_run: false,
        },
    })?;
    assert_eq!(code, 0);

    let hook_path = repo.join(".git").join("hooks").join("commit-msg");
    let hook_body = fs::read_to_string(&hook_path)?;
    assert!(hook_body.contains("Co-authored-by"));
    assert!(hook_body.contains("chatgpt|claude|codex|gemini|copilot|kiro|openai|anthropic"));

    #[cfg(unix)]
    {
        let mode = fs::metadata(&hook_path)?.permissions().mode();
        assert_ne!(mode & 0o111, 0);
    }

    Ok(())
}

#[test]
fn install_commit_guard_refuses_overwrite_without_force() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let repo = temp.path().join("repo");
    let hooks = repo.join(".git").join("hooks");
    fs::create_dir_all(&hooks)?;
    let hook_path = hooks.join("commit-msg");
    fs::write(&hook_path, "# existing hook\n")?;

    let result = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::InstallCommitGuard {
            repo: repo.clone(),
            force: false,
            dry_run: false,
        },
    });
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&hook_path)?, "# existing hook\n");

    Ok(())
}

#[test]
fn link_skills_sets_creates_hardlinks() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source_root = temp.path().join("skills");
    let skill_dir = source_root.join("my-skill");
    fs::create_dir_all(&skill_dir)?;
    let source_file = skill_dir.join("SKILL.md");
    fs::write(&source_file, "skill content")?;

    let target_root = temp.path().join("target");

    let source_str = path_str(&source_root);
    let target_str = path_str(&target_root);

    let config = format!(
        r#"[[skills_sets]]
source_root = "{}"
target_roots = ["{}"]
"#,
        source_str, target_str
    );
    fs::write(temp.path().join("prompt-sync.toml"), config)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);

    let target_file = target_root.join("my-skill").join("SKILL.md");
    assert!(target_file.exists(), "target skill file should exist");
    assert_eq!(fs::read_to_string(&target_file)?, "skill content");

    #[cfg(unix)]
    {
        let source_meta = fs::metadata(&source_file)?;
        let target_meta = fs::metadata(&target_file)?;
        assert_eq!(source_meta.ino(), target_meta.ino());
        assert_eq!(source_meta.dev(), target_meta.dev());
    }

    Ok(())
}

#[test]
fn link_skills_sets_exclude_filters_files() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source_root = temp.path().join("skills");

    // Create skill with references/ subdir that should be excluded
    let skill_dir = source_root.join("my-skill");
    fs::create_dir_all(skill_dir.join("references"))?;
    fs::write(skill_dir.join("SKILL.md"), "skill content")?;
    fs::write(skill_dir.join("references").join("ref.md"), "ref content")?;

    // Create another skill without references
    let skill2_dir = source_root.join("other-skill");
    fs::create_dir_all(&skill2_dir)?;
    fs::write(skill2_dir.join("SKILL.md"), "other content")?;

    let target_root = temp.path().join("target");
    let source_str = path_str(&source_root);
    let target_str = path_str(&target_root);

    let config = format!(
        r#"[[skills_sets]]
source_root = "{}"
target_roots = ["{}"]
exclude = ["*/references/**"]
"#,
        source_str, target_str
    );
    fs::write(temp.path().join("prompt-sync.toml"), config)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);

    // SKILL.md files should be linked
    assert!(target_root.join("my-skill").join("SKILL.md").exists());
    assert!(target_root.join("other-skill").join("SKILL.md").exists());

    // references/ should be excluded
    assert!(
        !target_root
            .join("my-skill")
            .join("references")
            .join("ref.md")
            .exists(),
        "references/ref.md should be excluded"
    );

    Ok(())
}

#[test]
fn link_skills_sets_only_skills_filters_dirs() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source_root = temp.path().join("skills");

    // Create three skills
    for name in &["alpha", "beta", "gamma"] {
        let dir = source_root.join(name);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("SKILL.md"), format!("{name} content"))?;
    }

    let target_root = temp.path().join("target");
    let source_str = path_str(&source_root);
    let target_str = path_str(&target_root);

    let config = format!(
        r#"[[skills_sets]]
source_root = "{}"
target_roots = ["{}"]
only_skills = ["alpha", "gamma"]
"#,
        source_str, target_str
    );
    fs::write(temp.path().join("prompt-sync.toml"), config)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);

    assert!(target_root.join("alpha").join("SKILL.md").exists());
    assert!(target_root.join("gamma").join("SKILL.md").exists());
    assert!(
        !target_root.join("beta").join("SKILL.md").exists(),
        "beta should be excluded by only_skills"
    );

    Ok(())
}

#[test]
fn link_skills_sets_exclude_skills_filters_dirs() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let source_root = temp.path().join("skills");

    // Create three skills
    for name in &["alpha", "beta", "gamma"] {
        let dir = source_root.join(name);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("SKILL.md"), format!("{name} content"))?;
    }

    let target_root = temp.path().join("target");
    let source_str = path_str(&source_root);
    let target_str = path_str(&target_root);

    let config = format!(
        r#"[[skills_sets]]
source_root = "{}"
target_roots = ["{}"]
exclude_skills = ["beta"]
"#,
        source_str, target_str
    );
    fs::write(temp.path().join("prompt-sync.toml"), config)?;

    let link_code = run(Cli {
        config: temp.path().join("prompt-sync.toml"),
        verbose: false,
        command: Command::Link {
            only_missing: false,
            force: false,
            dry_run: false,
            json: false,
            backup_dir: None,
        },
    })?;
    assert_eq!(link_code, 0);

    assert!(target_root.join("alpha").join("SKILL.md").exists());
    assert!(target_root.join("gamma").join("SKILL.md").exists());
    assert!(
        !target_root.join("beta").join("SKILL.md").exists(),
        "beta should be excluded by exclude_skills"
    );

    Ok(())
}

fn write_config(root: &Path, source: &Path, target: &Path) -> anyhow::Result<()> {
    // Convert paths to string, replacing backslashes with forward slashes for TOML compatibility
    let source_str = path_str(source);
    let target_str = path_str(target);

    let config = format!(
        r#"[[links]]
source = "{}"
targets = ["{}"]
"#,
        source_str, target_str
    );
    fs::write(root.join("prompt-sync.toml"), config)?;
    Ok(())
}

fn path_str(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}
