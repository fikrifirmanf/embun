#!/bin/sh
# Compares Embun against every capture under reference/.
# Slider settings map to Embun glass levels: clear 0, middle 0.5, tinted 1.
# Writes a <name>-compare.png next to each capture.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
cargo build -q -p embun-render --example compare
for scene in reference/*/*/*-scene.txt; do
  [ -e "$scene" ] || { echo "no captures yet; run tools/capture/capture.sh first" >&2; exit 1; }
  case "$scene" in
    */clear/*) level=0 ;;
    */middle/*) level=0.5 ;;
    */tinted/*) level=1 ;;
    *) echo "skipping $scene: folder is not clear, middle or tinted" >&2; continue ;;
  esac
  target/debug/examples/compare --capture "${scene%-scene.txt}" --level "$level"
  echo
done
