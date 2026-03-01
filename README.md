# prompt-sync

> **The ultimate synchronization tool for AI instruction files across multiple coding platforms.**
> Keep your AI instructions in sync, eliminate attribution clutter, and maintain a single source of truth.

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/pikafumi/prompt-sync/rust.yml?branch=main)](https://github.com/pikafumi/prompt-sync/actions)

![prompt-sync demo](./assets/prompt-sync-demo.gif)

---

## 🎯 The Problem

When working with multiple AI coding tools (Claude, Copilot, Gemini, Codex, etc.), teams face critical issues:

| Issue | Impact | prompt-sync Solution |
|-------|--------|-------------------|
| **Instruction Drift** | Different `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` get out of sync | Hard-link synchronization |
| **Commit Noise** | AI tool signatures pollute git history | Auto-removal commit hook |
| **No Single Source** | Manual updates across multiple locations | Centralized config + auto-sync |

**Real scenario:** You update your main instruction file, but the copy in `.github/copilot-instructions.md` doesn't update. Suddenly, Copilot uses stale instructions while Claude has the new version.

---

## ✨ Why prompt-sync

`prompt-sync` is a **production-grade Rust CLI** that brings order to multi-tool AI development:

- ✅ **One source of truth** for AI instruction files across all tools
- ✅ **Hard-link synchronization** for instant, zero-copy updates
- ✅ **Auto-clean commits** with built-in commit-msg hook
- ✅ **Enterprise-grade safety** with SHA256 backups, disk checks, and audit logs
- ✅ **Team-friendly** with reproducible configurations
- ✅ **Lightning-fast** (<50ms overhead per operation)

---

## 🚀 Quick Start

### Installation

```bash
cargo install prompt-sync
```

Or build from source:

```bash
git clone https://github.com/pikafumi/prompt-sync.git
cd prompt-sync
cargo build --release
./target/release/prompt-sync --help
```

### One-Command Setup

In your repository root:

```bash
# 1. Bootstrap with standard config
prompt-sync bootstrap --write-config

# 2. Install commit guard
prompt-sync install-commit-guard --repo .

# ✅ Done! Your AI instruction files are now synchronized
```

That's it. All your instruction files are now synchronized across all tools.

---

## 📋 Features

### Core Commands

| Command | Purpose | Example |
|---------|---------|---------|
| **`init`** | Generate starter config | `prompt-sync init --profile claude` |
| **`bootstrap`** | One-tap setup for common paths | `prompt-sync bootstrap --write-config` |
| **`link`** | Create/update hard links | `prompt-sync link --force` |
| **`verify`** | Check link health (OK/MISSING/BROKEN/CONFLICT) | `prompt-sync verify --json` |
| **`repair`** | Fix broken or missing links | `prompt-sync repair --force` |
| **`status`** | Quick health summary | `prompt-sync status` |
| **`install-commit-guard`** | Auto-clean git commits | `prompt-sync install-commit-guard` |

### 🔒 Advanced Safety Features (Enterprise-Grade)

#### 1. **SHA256 Integrity Verification**
Every backup file includes a SHA256 hash stored in metadata:
```
backups/
├── master-1707686700.bak
├── master-1707686700.sha256  ← Contains algorithm, hash, size, timestamp
```

#### 2. **Pre-flight Disk Space Check**
Before any replacement, prompt-sync verifies available disk space:
- ✅ Prevents incomplete backups
- ✅ Catches out-of-space errors early
- ✅ Supports Unix/Linux, macOS, Windows

#### 3. **Automatic Version Cleanup**
Intelligently manages backup versions:
- Keeps up to **100 backup versions** per file
- Auto-deletes oldest versions when limit exceeded
- Prevents unbounded storage growth

#### 4. **JSON Operation Logging**
Every file replacement logged in JSON for audit trails. The file is rewritten as a JSON array each write, so it is safe but not append-only:
```json
{
  "timestamp": "2026-02-12T11:45:00Z",
  "action": "replace",
  "source": "/path/to/source",
  "target": "/path/to/target",
  "status": "success",
  "hash_before": "abc123def456...",
  "backup_location": "/backups/target-1707686700.bak"
}
```

---

## 📦 Configuration

### Basic Config (`prompt-sync.toml`)

```toml
# Centralized instruction file synced across tools
[[links]]
source = "~/.ai_settings/master.md"
targets = [
  "~/.codex/AGENTS.md",
  "~/.claude/CLAUDE.md",
  "~/.gemini/GEMINI.md",
  "<repo>/.github/copilot-instructions.md",
]

# Skills synchronized across Claude/Gemini/Codex
[[skills_sets]]
source_root = "~/.agents/skills"
target_roots = [
  "~/.claude/skills",
  "~/.gemini/skills",
  "~/.codex/skills",
]
```

### Copilot Prompts & Instructions

Copilot does not use a skills directory. Instead, it reads `.github/prompts/*.prompt.md` and `.github/instructions/*.instructions.md`. You can manage these files via additional `skills_sets` entries with `mapping_kind` set to the appropriate variant (`PromptFile` or `InstructionFile`), or by adding them as individual `[[links]]` entries.

### Advanced: Backup Configuration

Protect critical operations with automated backups:

```bash
prompt-sync link --force --backup-dir ~/.prompt-sync/backups
```

Creates:
- `.bak` files with timestamped names
- `.sha256` hash metadata for integrity
- `.operations.log` with full audit trail
- Auto-cleanup after 100 versions

---

## 🎮 Command Examples

### Link Management

```bash
# Create new links from config
prompt-sync link

# Replace existing files safely
prompt-sync link --force

# Preview changes without applying
prompt-sync link --dry-run

# Only create missing links (skip conflicts)
prompt-sync link --only-missing

# With automatic backup
prompt-sync link --force --backup-dir ~/.prompt-sync/backups
```

### Verification & Repair

```bash
# Check current link health
prompt-sync verify

# JSON output for CI/CD pipelines
prompt-sync verify --json

# Repair broken links
prompt-sync repair

# Repair, replacing conflicts
prompt-sync repair --force

# Dry-run before actual repair
prompt-sync repair --force --dry-run
```

### Git Integration

```bash
# Install commit-msg hook
prompt-sync install-commit-guard --repo .

# Hook automatically:
# - Removes "Signed by Claude" lines
# - Cleans up "Generated by AI" markers
# - Preserves all real commit content
```

---

## 📊 How It Works

### Hard Links vs Symlinks

| Aspect | Hard Link | Symlink |
|--------|-----------|---------|
| **Visibility** | Same inode, perfect mirror | Separate file pointing elsewhere |
| **Performance** | Zero-copy, instant | Dereference overhead |
| **Tool Support** | 100% reliable across tools | May not work in all tools |
| **Storage** | No extra space used | Minimal overhead |

Hard links ensure when one tool updates the file, **all other tools instantly see the change**.

### Safety Pipeline

```
1. Pre-flight Check
   └─ Verify disk space
   └─ Check permissions

2. Backup Phase
   └─ Move old file to backup dir
   └─ Calculate SHA256 hash
   └─ Save metadata

3. Cleanup Phase
   └─ Remove versions >100
   └─ Clean orphaned hashes

4. Link Creation
   └─ Create hard link
   └─ Verify inode match

5. Audit Logging
   └─ Record in JSON log
   └─ Include error details
```

---

## ⚙️ Installation & Usage

### System Requirements

- **OS:** macOS, Linux, Windows (WSL)
- **Rust:** 1.70 or later
- **Disk:** <10 MB for binary

### Build from Source

```bash
git clone https://github.com/pikafumi/prompt-sync.git
cd prompt-sync

cargo build --release

# Optional: Install system-wide
sudo cp target/release/prompt-sync /usr/local/bin/
```

### Verify Installation

```bash
prompt-sync --version
prompt-sync --help
```

---

## 🔐 Safety & Reliability

### Backup & Recovery

All file replacements are safely backed up:

```
~/.prompt-sync/backups/
├── .operations.log           # Full audit trail (rewritten JSON array)
├── .operations.log.1         # Auto-rotated (1MB)
├── CLAUDE.md-1707686700.bak
├── CLAUDE.md-1707686700.sha256
├── AGENTS.md-1707686605.bak
└── AGENTS.md-1707686605.sha256
```

### Dry-Run Mode

Preview all changes before applying:

```bash
prompt-sync link --force --dry-run
prompt-sync repair --force --dry-run
```

### Permission & Config Safety

- Existing config files protected (use `--force` to override)
- Existing hooks protected (use `--force` to override)
- All operations logged and reversible

---

## 📈 Performance

Impact on typical workflow:

| Operation | Time | Impact |
|-----------|------|--------|
| Hard link creation | <1ms | Negligible |
| SHA256 hash (1MB) | 5-10ms | One-time |
| Disk space check | <1ms | Negligible |
| Cleanup (100 files) | <50ms | Auto |
| Git hook execution | <10ms | Per-commit |

**Overall:** File replacement overhead typically <50ms.

---

## 🛠️ Troubleshooting

### `ERROR: target is a directory; refusing to replace`

**Cause:** Source or target path points to a directory.

**Solution:** Update `prompt-sync.toml` to use file paths. Use `skills_sets` for directories.

### `ERROR: insufficient disk space`

**Cause:** Not enough free disk space for backup.

**Solution:**
```bash
df -h                                      # Check space
rm -rf ~/.prompt-sync/backups               # Clean backups
```

### `ERROR: hardlink across filesystems`

**Cause:** Source and target on different filesystems.

**Solution:** Configure targets on same filesystem, or use symlinks.

### Git hook not working

**Cause:** Hook not installed or disabled.

**Solution:**
```bash
prompt-sync install-commit-guard --repo . --force
cat .git/hooks/commit-msg  # Verify installation
```

---

## 🤝 Contributing

Contributions welcome!

### Development Setup

```bash
git clone https://github.com/pikafumi/prompt-sync.git
cd prompt-sync

# Run tests
cargo test

# Check code quality
cargo clippy

# Format code
cargo fmt
```

### Testing

```bash
# All tests
cargo test

# Specific test
cargo test link_then_verify_success

# With output
cargo test -- --nocapture
```

### Reporting Issues

Include:
1. OS and Rust version (`rustc --version`)
2. prompt-sync version (`prompt-sync --version`)
3. Full command and output
4. Relevant `prompt-sync.toml` (sanitized)

---

## 📚 Use Cases

### 1. **Multi-Tool AI Setup**
- **Problem:** Claude, Copilot, Gemini have different instructions
- **Solution:** `prompt-sync link` keeps all in sync
- **Result:** Single source of truth, no drift

### 2. **Team Repositories**
- **Problem:** CI/CD uses stale instructions
- **Solution:** Bootstrap once, auto-sync forever
- **Result:** Consistent AI behavior across team

### 3. **Compliance & Audit**
- **Problem:** "What instructions was the AI using?"
- **Solution:** JSON logs with timestamps and hashes
- **Result:** Complete audit trail

### 4. **Version Management**
- **Problem:** Need to roll back to previous instructions
- **Solution:** 100-version backup history with hashing
- **Result:** Easy recovery

---

## 📋 Command Reference

### Global Options

```bash
prompt-sync [OPTIONS] COMMAND

OPTIONS:
  -c, --config <FILE>    Path to prompt-sync.toml [default: ./prompt-sync.toml]
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
  --version              Print version
```

### All Commands

```bash
prompt-sync init                     # Generate starter config
prompt-sync bootstrap                # One-tap setup
prompt-sync link                     # Create/update links
prompt-sync verify                   # Check health
prompt-sync repair                   # Fix issues
prompt-sync status                   # Quick summary
prompt-sync install-commit-guard     # Git integration
```

### Common Patterns

```bash
# Safe: preview before applying
prompt-sync link --dry-run

# Production: with backups
prompt-sync link --force --backup-dir ~/.prompt-sync/backups

# CI/CD: JSON output
prompt-sync verify --json | jq .

# Recovery: check audit log (JSON array)
cat ~/.prompt-sync/backups/.operations.log | jq '.[0]'
```

---

## 🔍 Verify Hard Links

After running `prompt-sync link`, verify with:

### macOS/Linux

```bash
# Check inode numbers (should be identical)
ls -i ~/.ai_settings/master.md ~/.codex/AGENTS.md

# Output example:
# 12345678 ~/.ai_settings/master.md
# 12345678 ~/.codex/AGENTS.md  ← Same inode = properly linked
```

### Using prompt-sync

```bash
prompt-sync verify
# Output:
# [Ok] ~/.ai_settings/master.md -> ~/.codex/AGENTS.md
# [Ok] ~/.ai_settings/master.md -> ~/.claude/CLAUDE.md
```

---

## 📦 What's Included

- ✅ Binary CLI tool (Rust, production-ready)
- ✅ Configuration parser (TOML)
- ✅ Hard link management (cross-platform)
- ✅ Git commit-msg hook
- ✅ Backup & recovery system
- ✅ SHA256 integrity verification
- ✅ JSON audit logging
- ✅ Automated version cleanup
- ✅ Full test suite

---

## License

This project is dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.


