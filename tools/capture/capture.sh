#!/bin/sh
# Captures real macOS glass for one setting of the system glass slider.
# Usage: tools/capture/capture.sh clear|middle|tinted [shapes|controls|all] [active|inactive]
# Run it once per slider setting. See tools/capture/README.md.
# The controls scene writes controls-<backdrop>-<appearance> files.
# Active-window captures go to reference/macos-<ver>/<setting>/, inactive ones
# to reference/macos-<ver>/inactive/<setting>/.
set -eu

setting="${1:-}"
case "$setting" in
  clear|middle|tinted) ;;
  *) echo "usage: $0 clear|middle|tinted [shapes|controls|all] [active|inactive]" >&2; exit 2 ;;
esac
if [ "${2:-}" = all ]; then
  sh "$0" "$setting" shapes "${3:-active}"
  exec sh "$0" "$setting" controls "${3:-active}"
fi
scene="${2:-shapes}"
case "$scene" in
  shapes) prefix="" ;;
  controls) prefix="controls-" ;;
  *) echo "usage: $0 clear|middle|tinted [shapes|controls|all] [active|inactive]" >&2; exit 2 ;;
esac
window="${3:-active}"
case "$window" in
  active) state_dir=""; state_flag="" ;;
  inactive) state_dir="inactive/"; state_flag="--inactive" ;;
  *) echo "usage: $0 clear|middle|tinted [shapes|controls|all] [active|inactive]" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
version="$(sw_vers -productVersion)"
out="reference/macos-$version/$state_dir$setting"
build="target/capture"
mkdir -p "$out" "$build"

xcrun swiftc -O -swift-version 5 -o "$build/EmbunCapture" tools/capture/main.swift
cargo run -q -p embun-render --example export_backdrops -- "$build/backdrops" >/dev/null

for appearance in light dark; do
  flag=""
  [ "$appearance" = dark ] && flag="--dark"
  for backdrop in photos checker solid noise; do
    name="$prefix$backdrop-$appearance"
    "$build/EmbunCapture" --backdrop "$build/backdrops/$backdrop-$appearance.png" --out "$out/$name" --scene "$scene" $flag $state_flag
    echo "captured $out/$name"
  done
done

about="$out/about.txt"
[ "$scene" = controls ] && about="$out/about-controls.txt"
{
  echo "macOS $version ($(sw_vers -buildVersion))"
  echo "glass slider: $setting"
  echo "window: $window"
  echo "captured: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$about"
echo "done: $out"
