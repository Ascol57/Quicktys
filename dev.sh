#!/usr/bin/env bash
# Quicktys dev launcher — rebuild extensions, (re)pack + sign the marketplace
# catalogue, serve it, and run the app against it, all in one.
#
#   ./dev.sh              # full: rebuild bundles + frontend, pack, serve, run
#   PORT=9000 ./dev.sh    # serve the registry on a different port
#   SKIP_BUNDLES=1 ./dev.sh   # skip the (slow) wasm bundle rebuild
#   SKIP_FRONTEND=1 ./dev.sh  # skip the frontend rebuild
#
# `pear` is packaged as a premium (paid + encrypted) example; a dev CA + store
# key live under dist/ and a matching license is written to dist/pear.license.
set -euo pipefail
cd "$(dirname "$0")"

PORT="${PORT:-8000}"
CA=dist/ca.key
STORE=dist/store.key

step() { printf '\n\033[1;36m▸ %s\033[0m\n' "$1"; }

# --- 1. Rebuild the extension bundles (fresh wasm against the current WIT) ---
if [[ -z "${SKIP_BUNDLES:-}" ]]; then
  step "Building extension bundles (wasm)…"
  scripts/build-bundles.sh
else
  step "Skipping bundle rebuild (SKIP_BUNDLES set)"
fi

# --- 2. Rebuild the frontend (the app embeds frontend/dist at build time) ----
if [[ -z "${SKIP_FRONTEND:-}" ]]; then
  step "Building frontend…"
  npm --prefix frontend run build
fi

# --- 3. Pack + sign the catalogue (pear = premium/encrypted) -----------------
step "Packing + signing the marketplace catalogue…"
cargo run -q -p quicktys-pack -- dist/bundles --out dist/pkgs \
  --sign --ca "$CA" --encrypt module/pear:pear-pro >/dev/null
# Issue the pear license (stable content key → valid across re-packs).
cargo run -q -p quicktys-pack -- --issue-license module/pear:pear-pro --store "$STORE" \
  2>/dev/null | grep '^QTLIC-' > dist/pear.license
echo "  license → dist/pear.license   (paste it in the app to unlock pear)"

# --- 4. Clean regenerable / stale artifacts ---------------------------------
step "Cleaning stale artifacts…"
rm -rf dist/registry                      # regserve's regenerable output dir
rm -rf dist/pkgs/.tmp 2>/dev/null || true
# Stale cached registry index/CRL in the app data dir (refreshed on next run).
rm -f "$HOME/.local/share/quicktys/registry-cache/"*.json 2>/dev/null || true

# --- 5. Build binaries up front so errors surface before we fork ------------
step "Building app + registry server…"
cargo build -q -p quicktys-regserve -p quicktys-app

# --- 6. Serve the registry in the background; kill it on exit ----------------
step "Serving registry on http://127.0.0.1:$PORT"
cargo run -q -p quicktys-regserve -- dist/pkgs --port "$PORT" --out dist/registry &
SERVER=$!
trap 'echo; echo "stopping registry ($SERVER)…"; kill "$SERVER" 2>/dev/null || true' EXIT INT TERM
sleep 1

# --- 7. Launch the app against the registry (clean boot, keys pinned) --------
step "Launching Quicktys…"
QUICKTYS_NO_DEV_BUNDLES=1 \
QUICKTYS_REGISTRY="http://127.0.0.1:$PORT" \
QUICKTYS_CA="$(cat dist/ca.pub)" \
QUICKTYS_STORE="$(cat dist/store.pub)" \
cargo run -p quicktys-app
