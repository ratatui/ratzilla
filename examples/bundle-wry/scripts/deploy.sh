#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

usage() {
  cat <<'EOF'
Usage: scripts/deploy.sh [--out-dir DIR] [--tag TAG] [--skip-build]

Builds the desktop wrapper and wasm app, then stages release artifacts under
the chosen output directory. If --tag is provided, the archives are uploaded to
the matching GitHub release with the gh CLI.
EOF
}

out_dir="dist/releases"
tag=""
skip_build=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out-dir)
      out_dir="$2"
      shift 2
      ;;
    --tag)
      tag="$2"
      shift 2
      ;;
    --skip-build)
      skip_build=true
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

root="$(repo_root)"
cd "$root"
mkdir -p "$out_dir"

if [[ "$skip_build" == false ]]; then
  "$root/scripts/bundle.sh" --build
fi

host="$(host_triple)"
release_dir="$out_dir/$host"
bundle_root="$(cargo_target_dir)/release/bundle"
wasm_dist="dist"

bundle_artifact="$(bundle_artifact_path bundle-wry "$bundle_root")"

if [[ -z "$bundle_artifact" ]]; then
  echo "Missing bundle-wry bundle in $bundle_root; omit --skip-build to build it." >&2
  exit 1
fi

if [[ ! -d "$wasm_dist" ]]; then
  echo "Missing $wasm_dist; omit --skip-build to build it." >&2
  exit 1
fi

rm -rf "$release_dir"
mkdir -p "$release_dir"

bundle_name="$(basename "$bundle_artifact")"

cp -R "$bundle_artifact" "$release_dir/$bundle_name"
cp -R "$wasm_dist" "$release_dir/bundle-wry-web"

bundle_archive="$out_dir/bundle-wry-$host.tar.gz"
web_archive="$out_dir/bundle-wry-web-$host.tar.gz"

tar -C "$release_dir" -czf "$bundle_archive" "$bundle_name"
tar -C "$release_dir" -czf "$web_archive" bundle-wry-web

printf 'Staged %s\n' "$release_dir"
printf 'Packed %s\n' "$bundle_archive"
printf 'Packed %s\n' "$web_archive"

if [[ -n "$tag" ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "gh is required to upload release artifacts." >&2
    exit 1
  fi

  if ! gh release view "$tag" >/dev/null 2>&1; then
    gh release create "$tag" --title "$tag" --notes "Automated release for $tag"
  fi

  gh release upload "$tag" "$bundle_archive" "$web_archive" --clobber
fi
