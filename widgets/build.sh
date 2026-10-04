#!/usr/bin/env bash
# Build every widget to foster-server/static/widgets/<name>/<name>_widget.js
# (or $WIDGETS_OUT/<name>/)
# (+ _bg.wasm), where index.html's fx-widget attributes point. Needs
# wasm-pack and the wasm32-unknown-unknown target.
set -euo pipefail
cd "$(dirname "$0")"
out_root=$(realpath -m "${WIDGETS_OUT:-../foster-server/static/widgets}")
for crate in */; do
  crate=${crate%/}
  [[ $crate == common || ! -f $crate/Cargo.toml ]] && continue
  echo "▸ widget: $crate"
  rm -rf "$out_root/$crate"
  wasm-pack build "$crate" --release --target web --no-typescript --no-pack --out-dir "$out_root/$crate"
  rm -f "$out_root/$crate/.gitignore"
done
