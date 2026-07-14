#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

usage() {
  cat <<'EOF'
Usage: scripts/bundle.sh [--build] [--run] [--test]

Builds the macOS app bundle for bundle-wry.
--run opens the bundled macOS app.
--test opens the bundled macOS app, curls the bundled index.html, and verifies the HTML.
EOF
}

build=false
run=false
test=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build)
      build=true
      shift
      ;;
    --run)
      run=true
      shift
      ;;
    --test)
      test=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$build" == false && "$run" == false && "$test" == false ]]; then
  usage
  exit 1
fi

root="$(repo_root)"
cd "$root"

bundle_root="$(cargo_target_dir)/release/bundle"
bundle_name="bundle-wry"
bundle_app_path() {
  find "$bundle_root" -type d -name "${bundle_name}.app" -print -quit
}

build_bundle() {
  ensure_wasm_target
  ensure_cargo_bundle
  ensure_trunk

  trunk build --release --dist dist
  cargo bundle --bin bundle-wry --release
}

ensure_bundle() {
  if [[ -z "$(bundle_app_path)" ]]; then
    build_bundle
  fi
}

run_bundle() {
  local app_path
  app_path="$(bundle_app_path)"
  if [[ -z "$app_path" ]]; then
    echo "Missing bundled app; run scripts/bundle.sh --build first." >&2
    exit 1
  fi

  open "$app_path"
}

test_bundle() {
  local app_path
  local html
  local app_pid=""
  local url="http://127.0.0.1:8080"

  app_path="$(bundle_app_path)"
  if [[ -z "$app_path" ]]; then
    echo "Missing bundled app; run scripts/bundle.sh --build first." >&2
    exit 1
  fi

  if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
    echo "Port 8080 is already in use; stop the existing listener first." >&2
    exit 1
  fi

  cleanup() {
    if [[ -n "$app_pid" ]] && kill -0 "$app_pid" >/dev/null 2>&1; then
      kill "$app_pid" >/dev/null 2>&1 || true
      wait "$app_pid" >/dev/null 2>&1 || true
    fi
  }

  trap cleanup EXIT

  open "$app_path"

  for _ in $(seq 1 60); do
    app_pid="$(pgrep -n -f "${bundle_name}.app/Contents/MacOS/${bundle_name}" || true)"
    if [[ -n "$app_pid" ]]; then
      break
    fi
    sleep 1
  done

  if [[ -z "$app_pid" ]]; then
    echo "Timed out waiting for bundled app process to start." >&2
    exit 1
  fi

  for _ in $(seq 1 60); do
    if html="$(curl -fsS "$url" 2>/dev/null)"; then
      break
    fi
    sleep 1
  done

  if [[ -z "${html:-}" ]]; then
    echo "Timed out waiting for $url" >&2
    exit 1
  fi

  printf '%s' "$html" | grep -F '<title>Ratzilla Demo</title>'
  js_path="$(printf '%s' "$html" | grep -oE "./[^']+\.js" | head -n 1)"
  wasm_path="$(printf '%s' "$html" | grep -oE "./[^']+_bg\.wasm" | head -n 1)"
  curl -fsS "$url/${js_path#./}" >/dev/null
  curl -fsS "$url/${wasm_path#./}" >/dev/null

  cleanup
  trap - EXIT
  printf 'Verified %s\n' "$url"
}

if [[ "$build" == true ]]; then
  build_bundle
fi

if [[ "$run" == true ]]; then
  ensure_bundle
  run_bundle
fi

if [[ "$test" == true ]]; then
  ensure_bundle
  test_bundle
fi
