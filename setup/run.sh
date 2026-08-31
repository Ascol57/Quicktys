#!/usr/bin/env bash
# Quicktys launcher — macOS & Linux.
#
#   ./setup/run.sh                # build everything, start the registry server
#                                 # + the desktop app (the default: server+client)
#   ./setup/run.sh --solo         # the app alone, no marketplace server
#   ./setup/run.sh --web          # the frontend alone, in a browser (Vite dev)
#   ./setup/run.sh --skip-build   # don't rebuild bundles/frontend, just launch
#   ./setup/run.sh --port 9000    # registry server port (default 8000)
#
# What "server" and "client" mean here:
#   * client  — the Tauri desktop app (`quicktys-app`). It embeds the built
#               frontend, and serves overlays for OBS on its own local port.
#   * server  — `quicktys-regserve`, the marketplace/registry the app installs
#               extensions from. Optional: without it the app still loads the
#               locally built bundles from dist/bundles.
set -euo pipefail
. "$(cd "$(dirname "$0")" && pwd)/_common.sh"
cd "$REPO_ROOT"

MODE="full"
SKIP_BUNDLES=""
SKIP_FRONTEND=""
PORT="${PORT:-8000}"

while [ $# -gt 0 ]; do
  case "$1" in
    --solo) MODE="solo" ;;
    --web) MODE="web" ;;
    --skip-build) SKIP_BUNDLES=1; SKIP_FRONTEND=1 ;;
    --skip-bundles) SKIP_BUNDLES=1 ;;
    --skip-frontend) SKIP_FRONTEND=1 ;;
    --port) shift; PORT="${1:?--port needs a value}" ;;
    -h | --help) usage_from_header "$0"; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
  shift
done

OS="$(detect_os)"
load_toolchains

# --- Prerequisites -----------------------------------------------------------
step "Checking prerequisites"
if ! check_prereqs; then
  case "$OS" in
    macos) die "Missing prerequisites — run: ./setup/setup-macos.sh" ;;
    linux) die "Missing prerequisites — run: ./setup/setup-linux.sh" ;;
    *) die "Missing prerequisites — see setup/README.md" ;;
  esac
fi

if [ ! -d frontend/node_modules ]; then
  step "Installing frontend dependencies (first run)"
  npm --prefix frontend install
fi

# --- Browser-only mode: no Rust build at all ---------------------------------
if [ "$MODE" = "web" ]; then
  step "Starting the Vite dev server (browser mock)"
  info "the app's IPC is mocked in the browser — no host, no extensions"
  exec npm --prefix frontend run dev
fi

# --- Build -------------------------------------------------------------------
if [ -z "$SKIP_BUNDLES" ]; then
  step "Building extension bundles (wasm)"
  info "first build compiles one component per extension — this is the slow part"
  scripts/build-bundles.sh
else
  info "skipping bundle rebuild"
fi

if [ -z "$SKIP_FRONTEND" ]; then
  step "Building the frontend"
  npm --prefix frontend run build
else
  info "skipping frontend rebuild"
fi

# --- Launch ------------------------------------------------------------------
if [ "$MODE" = "solo" ]; then
  step "Launching Quicktys (no registry server)"
  info "extensions come from dist/bundles"
  [ -d dist/bundles ] || warn "dist/bundles is missing — run without --skip-bundles"
  exec cargo run -p quicktys-app
fi

# Full mode signs the catalogue, so it needs the dev keypairs. Idempotent: this
# only creates what is missing (dist/ is gitignored, so a fresh clone has none).
if [ ! -f dist/ca.pub ] || [ ! -f dist/store.pub ]; then
  step "Generating the development keys"
  ./setup/gen-keys.sh
fi

# Full mode: dev.sh already packs + signs the catalogue, serves it, and runs the
# app against it. Reuse it rather than duplicating that flow here.
step "Launching registry server + app"
info "registry on http://127.0.0.1:$PORT — Ctrl-C stops both"
export PORT SKIP_BUNDLES SKIP_FRONTEND
# dev.sh rebuilds by default; we already did it above.
export SKIP_BUNDLES=1 SKIP_FRONTEND=1
exec ./dev.sh
