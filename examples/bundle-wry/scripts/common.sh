#!/usr/bin/env bash

set -euo pipefail

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

ensure_wasm_target() {
  if rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
    return
  fi

  rustup target add wasm32-unknown-unknown
}

ensure_trunk() {
  if command -v trunk >/dev/null 2>&1; then
    return
  fi

  cargo install trunk --locked
}

ensure_cargo_bundle() {
  if command -v cargo-bundle >/dev/null 2>&1; then
    return
  fi

  cargo install cargo-bundle --locked
}

cargo_target_dir() {
  local target_dir
  target_dir="$(
    cargo metadata --format-version 1 --no-deps |
      sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'
  )"

  if [[ -z "$target_dir" ]]; then
    echo "Unable to determine Cargo target directory." >&2
    return 1
  fi

  printf '%s\n' "$target_dir"
}

host_triple() {
  local line
  while IFS= read -r line; do
    case "$line" in
      host:*)
        printf '%s\n' "${line#host: }"
        return
        ;;
    esac
  done < <(rustc -vV)

  return 1
}

bundle_artifact_path() {
  local binary_name="$1"
  local bundle_root="${2:-$(cargo_target_dir)/release/bundle}"
  local os_name
  os_name="$(uname -s)"

  case "$os_name" in
    Darwin)
      local artifact
      artifact="$(find "$bundle_root" -type f -name "${binary_name}.dmg" -print -quit)"
      if [[ -n "$artifact" ]]; then
        printf '%s\n' "$artifact"
        return
      fi
      find "$bundle_root" -type d -name "${binary_name}.app" -print -quit
      ;;
    Linux)
      local artifact
      artifact="$(find "$bundle_root" -type f -name "${binary_name}.deb" -print -quit)"
      if [[ -n "$artifact" ]]; then
        printf '%s\n' "$artifact"
        return
      fi
      find "$bundle_root" -type f -name "${binary_name}.AppImage" -print -quit
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      local artifact
      artifact="$(find "$bundle_root" -type f -name "${binary_name}.msi" -print -quit)"
      if [[ -n "$artifact" ]]; then
        printf '%s\n' "$artifact"
        return
      fi
      find "$bundle_root" -type f -name "${binary_name}.exe" -print -quit
      ;;
    *)
      find "$bundle_root" -type f -name "${binary_name}*" -print -quit
      ;;
  esac
}
