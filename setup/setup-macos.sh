#!/usr/bin/env bash
# Quicktys — one-shot prerequisites install for macOS (Intel & Apple Silicon).
#
#   ./setup/setup-macos.sh          # ask before installing anything
#   ASSUME_YES=1 ./setup/setup-macos.sh
#
# Installs: Xcode Command Line Tools, Homebrew (optional), Node, Rust +
# the wasm32-wasip2 target, and the frontend's npm packages.
#
# macOS needs NO extra system libraries for the webview itself: Tauri renders in
# the system WKWebView. The native dependencies that do get compiled — `rodio`
# (CoreAudio), `enigo` (CoreGraphics) and `hidapi` (IOKit) — only need the Xcode
# Command Line Tools, which is why step 1 is not optional.
set -euo pipefail
. "$(cd "$(dirname "$0")" && pwd)/_common.sh"

[ "$(detect_os)" = "macos" ] || die "This script is for macOS. Use setup/setup-linux.sh or setup/setup-windows.ps1."

printf '%sQuicktys — macOS setup%s\n' "$C_STEP" "$C_OFF"
info "repo: $REPO_ROOT"
load_toolchains

# --- 1. Xcode Command Line Tools (compiler + linker + system frameworks) -----
step "Xcode Command Line Tools"
if xcode-select -p >/dev/null 2>&1; then
  ok "installed ($(xcode-select -p))"
else
  warn "not installed — required to link CoreAudio / CoreGraphics / IOKit"
  if ask "Launch the installer now?"; then
    # This opens a GUI installer and returns immediately.
    xcode-select --install || true
    die "Finish the Xcode Command Line Tools install, then run this script again."
  else
    die "Cannot continue without the Command Line Tools (xcode-select --install)."
  fi
fi

# --- 2. Homebrew (only used to get Node; skip if you install Node yourself) ---
step "Homebrew"
if have brew; then
  ok "$(brew --version | head -n1)"
elif have node && have npm; then
  info "not installed, but Node is already available — skipping"
else
  warn "not installed, and it is the simplest way to get Node"
  if ask "Install Homebrew (runs the official install script from brew.sh)?"; then
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    load_toolchains
    ok "Homebrew installed"
  else
    info "Skipped — install Node yourself from https://nodejs.org before continuing"
  fi
fi

# --- 3. Node (the frontend is a Vue 3 + Vite app) ----------------------------
step "Node.js"
if have node && have npm && version_ge "$(node_version)" "$MIN_NODE"; then
  ok "Node $(node_version), npm $(npm --version)"
elif have brew; then
  if ask "Install Node via Homebrew?"; then
    brew install node
    ok "Node $(node_version)"
  else
    die "Node $MIN_NODE+ is required."
  fi
else
  die "Node $MIN_NODE+ is required — install it from https://nodejs.org and re-run."
fi

# --- 4. Rust (host workspace) + the wasm target (extensions) -----------------
step "Rust toolchain"
if have rustc && version_ge "$(rust_version)" "$MIN_RUST"; then
  ok "Rust $(rust_version)"
else
  if have rustup; then
    if ask "Rust is missing or too old. Run rustup update?"; then
      rustup update
      load_toolchains
    fi
  elif ask "Install Rust via rustup (rustup.rs)?"; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    load_toolchains
  fi
  have rustc && version_ge "$(rust_version)" "$MIN_RUST" \
    || die "Rust $MIN_RUST+ is required (got '$(rust_version)')."
  ok "Rust $(rust_version)"
fi

step "wasm32-wasip2 target (plugins, modules)"
if rustup target list --installed 2>/dev/null | grep -q '^wasm32-wasip2$'; then
  ok "already installed"
else
  rustup target add wasm32-wasip2
  ok "installed"
fi

# --- 5. Frontend packages ----------------------------------------------------
step "Frontend dependencies (npm)"
if [ -d "$REPO_ROOT/frontend/node_modules" ]; then
  ok "frontend/node_modules already present"
  info "re-run 'npm --prefix frontend install' if package.json changed"
else
  npm --prefix "$REPO_ROOT/frontend" install
  ok "installed"
fi

# --- 6. Development keys -----------------------------------------------------
# The certification authority + store keypairs. dist/ is gitignored, so a fresh
# clone has none and the signed-catalogue run cannot work without them.
step "Development keys"
if [ -f "$REPO_ROOT/dist/ca.pub" ] && [ -f "$REPO_ROOT/dist/store.pub" ]; then
  ok "already generated"
elif ask "Generate them now? (builds the packaging tool first — a few minutes)"; then
  "$REPO_ROOT/setup/gen-keys.sh"
else
  info "skipped — ./setup/run.sh will generate them on first launch"
fi

# --- Done --------------------------------------------------------------------
step "Ready"
check_prereqs || warn "some checks above still fail — see the messages"
cat <<EOF

  Next:  ./setup/run.sh            # build everything, then start server + app
         ./setup/run.sh --solo     # the app alone (no marketplace server)
         ./setup/run.sh --web      # frontend only, in your browser

  First build takes a while (the whole Rust workspace + one wasm component per
  extension). Later runs reuse target/ and are much faster.
EOF
