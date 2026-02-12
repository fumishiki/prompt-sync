#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT_GIF="$ROOT_DIR/assets/skillslink-demo.gif"
TAPE_FILE="$ROOT_DIR/demo/skillslink.tape"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo is required"
  exit 1
fi

mkdir -p "$ROOT_DIR/assets"
"$ROOT_DIR/scripts/prepare_demo_env.sh"

(
  cd "$ROOT_DIR"
  cargo build --release
)

run_local_vhs() {
  (
    cd "$ROOT_DIR"
    vhs "$TAPE_FILE"
  )
}

run_docker_vhs() {
  (
    cd "$ROOT_DIR"
    docker run --rm -v "$PWD":/vhs -w /vhs ghcr.io/charmbracelet/vhs demo/skillslink.tape
  )
}

if command -v vhs >/dev/null 2>&1; then
  if run_local_vhs; then
    echo "generated with local vhs: $OUT_GIF"
    exit 0
  fi
  echo "warn: local vhs failed, trying Docker fallback..."
fi

if command -v docker >/dev/null 2>&1; then
  run_docker_vhs
  echo "generated with Docker vhs: $OUT_GIF"
  exit 0
fi

echo "error: could not render GIF."
echo "Install one of:"
echo "  1) local vhs (macOS: brew install vhs)"
echo "  2) Docker Desktop (for ghcr.io/charmbracelet/vhs fallback)"
exit 1
