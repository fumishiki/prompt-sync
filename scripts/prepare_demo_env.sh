#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DEMO_DIR="$ROOT_DIR/.tmp_demo"
DEMO_REPO="$DEMO_DIR/repo"
CONFIG_PATH="$DEMO_DIR/skillslink.toml"

rm -rf "$DEMO_DIR"
mkdir -p "$DEMO_DIR/skills-master/rust"
mkdir -p "$DEMO_REPO"

cat > "$DEMO_DIR/master.md" <<'EOF'
# Demo Master Instructions

Update this file once and every linked instruction file updates together.
EOF

cat > "$DEMO_DIR/skills-master/rust/SKILL.md" <<'EOF'
# Rust Architect

Prefer zero-copy APIs and explicit error propagation.
EOF

cat > "$CONFIG_PATH" <<'EOF'
[[links]]
source = "<repo>/.tmp_demo/master.md"
targets = [
  "<repo>/.tmp_demo/AGENTS.md",
  "<repo>/.tmp_demo/CLAUDE.md",
  "<repo>/.tmp_demo/GEMINI.md",
  "<repo>/.tmp_demo/.github/copilot-instructions.md",
]

[[skills_sets]]
source_root = "<repo>/.tmp_demo/skills-master"
target_roots = [
  "<repo>/.tmp_demo/.claude/skills",
  "<repo>/.tmp_demo/.gemini/skills",
  "<repo>/.tmp_demo/.github/skills",
]
EOF

if [ ! -d "$DEMO_REPO/.git" ]; then
  git -c init.defaultBranch=main init -q "$DEMO_REPO"
fi

cat > "$DEMO_REPO/.git/COMMIT_EDITMSG" <<'EOF'
feat: demo commit guard

Co-authored-by: Codex <codex@openai.com>
Generated with ChatGPT
Reviewed-by: fumishiki <fumishiki@users.noreply.github.com>
EOF

echo "prepared demo workspace: $DEMO_DIR"
