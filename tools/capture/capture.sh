#!/bin/sh
# Captures real macOS glass for one setting of the system glass slider.
# Usage: tools/capture/capture.sh clear|middle|tinted
# Run it once per slider setting. See tools/capture/README.md.
set -eu

setting="${1:-}"
case "$setting" in
  clear|middle|tinted) ;;
  *) echo "usage: $0 clear|middle|tinted" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
version="$(sw_vers -productVersion)"
out="reference/macos-$version/$setting"
build="target/capture"
mkdir -p "$out" "$build"

xcrun swiftc -O -swift-version 5 -o "$build/EmbunCapture" tools/capture/main.swift
cargo run -q -p embun-render --example export_backdrops -- "$build/backdrops" >/dev/null

for appearance in light dark; do
  flag=""
  [ "$appearance" = dark ] && flag="--dark"
  for backdrop in photos checker solid; do
    name="$backdrop-$appearance"
    "$build/EmbunCapture" --backdrop "$build/backdrops/$name.png" --out "$out/$name" $flag
    echo "captured $out/$name"
  done
done

{
  echo "macOS $version ($(sw_vers -buildVersion))"
  echo "glass slider: $setting"
  echo "captured: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$out/about.txt"
echo "done: $out"
