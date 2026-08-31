#!/usr/bin/env bash
# Quicktys — one-shot prerequisites install for Linux.
#
#   ./setup/setup-linux.sh
#   ASSUME_YES=1 ./setup/setup-linux.sh
#
# Installs the GTK/WebKit stack Tauri needs, Node, Rust + the wasm32-wasip2
# target, and the frontend's npm packages. Supports apt (Debian/Ubuntu), dnf
# (Fedora/RHEL), pacman (Arch) and zypper (openSUSE); anything else gets the
# package list printed for you to translate.
set -euo pipefail
. "$(cd "$(dirname "$0")" && pwd)/_common.sh"

[ "$(detect_os)" = "linux" ] || die "This script is for Linux. Use setup/setup-macos.sh or setup/setup-windows.ps1."

printf '%sQuicktys — Linux setup%s\n' "$C_STEP" "$C_OFF"
info "repo: $REPO_ROOT"
load_toolchains

SUDO=""
[ "$(id -u)" != "0" ] && have sudo && SUDO="sudo"

# --- 1. System libraries (Tauri's webview + the crates that link C) ----------
step "System packages"
if have apt-get; then
  PKGS="build-essential pkg-config curl wget file libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev"
  INSTALL="$SUDO apt-get install -y $PKGS"
  UPDATE="$SUDO apt-get update"
elif have dnf; then
  PKGS="gcc gcc-c++ make pkgconf-pkg-config openssl-devel curl wget file gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel alsa-lib-devel"
  INSTALL="$SUDO dnf install -y $PKGS"
  UPDATE=""
elif have pacman; then
  PKGS="base-devel pkgconf openssl curl wget file gtk3 webkit2gtk-4.1 libappindicator-gtk3 librsvg alsa-lib"
  INSTALL="$SUDO pacman -S --needed --noconfirm $PKGS"
  UPDATE=""
elif have zypper; then
  PKGS="gcc gcc-c++ make pkg-config libopenssl-devel curl wget file gtk3-devel webkit2gtk3-soup2-devel libappindicator3-devel librsvg-devel alsa-devel"
  INSTALL="$SUDO zypper install -y $PKGS"
  UPDATE=""
else
  warn "unknown package manager — install the equivalents of:"
  info "  build tools, pkg-config, openssl, gtk3, webkit2gtk 4.1, libappindicator3, librsvg, alsa"
  INSTALL=""
  UPDATE=""
fi

if [ -n "$INSTALL" ]; then
  info "$INSTALL"
  if ask "Install these system packages?"; then
    [ -n "$UPDATE" ] && $UPDATE
    $INSTALL
    ok "system packages installed"
  else
    warn "skipped — the build will fail if webkit2gtk/gtk3 are missing"
  fi
fi

# --- 2. Rust + the wasm target ----------------------------------------------
step "Rust toolchain"
if have rustc && version_ge "$(rust_version)" "$MIN_RUST"; then
  ok "Rust $(rust_version)"
else
  if have rustup; then
    ask "Rust is missing or too old. Run rustup update?" && { rustup update; load_toolchains; }
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

# --- 3. Node -----------------------------------------------------------------
step "Node.js"
if have node && have npm && version_ge "$(node_version)" "$MIN_NODE"; then
  ok "Node $(node_version), npm $(npm --version)"
else
  warn "Node $MIN_NODE+ not found"
  if have apt-get && ask "Install the distro's nodejs + npm? (may be older than $MIN_NODE)"; then
    $SUDO apt-get install -y nodejs npm
  elif have dnf && ask "Install nodejs + npm via dnf?"; then
    $SUDO dnf install -y nodejs npm
  elif have pacman && ask "Install nodejs + npm via pacman?"; then
    $SUDO pacman -S --needed --noconfirm nodejs npm
  fi
  have node && have npm && version_ge "$(node_version)" "$MIN_NODE" \
    || die "Node $MIN_NODE+ is required — see https://nodejs.org or use nvm."
  ok "Node $(node_version)"
fi

# --- 4. Frontend packages ----------------------------------------------------
step "Frontend dependencies (npm)"
if [ -d "$REPO_ROOT/frontend/node_modules" ]; then
  ok "frontend/node_modules already present"
else
  npm --prefix "$REPO_ROOT/frontend" install
  ok "installed"
fi

# --- 5. Development keys -----------------------------------------------------
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

step "Ready"
check_prereqs || warn "some checks above still fail — see the messages"
cat <<EOF

  Next:  ./setup/run.sh            # build everything, then start server + app
         ./setup/run.sh --solo     # the app alone (no marketplace server)
         ./setup/run.sh --web      # frontend only, in your browser

  A GUI run needs a real desktop session (GTK/WebKit) — it will not start over
  SSH without a display.
EOF
