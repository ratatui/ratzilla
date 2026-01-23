#!/usr/bin/env bash

mkdir -p dist
for example in examples/*; do
  example_name=$(basename "$example")
  
  # Skip if not a directory
  if [ ! -d "$example" ]; then
    continue
  fi
  
  # Skip if no Cargo.toml (not a Rust project)
  if [ ! -f "$example/Cargo.toml" ]; then
    echo "Skipping $example_name: no Cargo.toml found"
    continue
  fi
  
  # Skip if no index.html (not a web example)  
  if [ ! -f "$example/index.html" ]; then
    echo "Skipping $example_name: no index.html found"
    continue
  fi
  
  # Skip shared library
  if [ "$example_name" == "shared" ]; then
    echo "Skipping $example_name: shared library"
    continue
  fi
  
  echo "Building $example_name..."
  mkdir -p dist/"$example_name"
  pushd "$example" || exit
  
  if [ "$example_name" == "website" ]; then
    trunk build --release --public-url https://ratatui.github.io/ratzilla/
    cp -r dist/* ../../dist/
  elif [[ "$example_name" == "tauri"* ]]; then
    echo "Skipping Tauri example"
  else
    trunk build --release --public-url https://ratatui.github.io/ratzilla/"$example_name"
    cp -r dist/* ../../dist/"$example_name"
  fi
  popd || exit
done
