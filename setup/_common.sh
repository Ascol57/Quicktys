#!/usr/bin/env bash
# Shared helpers for the setup/run scripts. Sourced, never executed directly.
#
# Kept compatible with **bash 3.2**, which is what macOS still ships as
# /bin/bash: no associative arrays, no ${var,,}, no readarray.

# Repo root, regardless of where the script was called from.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Minimum Rust for the `wasm32-wasip2` target the extensions are built for
# (added in Rust 1.82 — before that the target was called wasm32-wasi).
MIN_RUST="1.82.0"
# Vite 6 needs Node 18+; 20 LTS is what the frontend is developed against.
MIN_NODE="18.0.0"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_STEP=$'\033[1;36m'; C_OK=$'\033[1;32m'; C_WARN=$'\033[1;33m'
  C_ERR=$'\033[1;31m'; C_DIM=$'\033[2m'; C_OFF=$'\033[0m'
else
  C_STEP=""; C_OK=""; C_WARN=""; C_ERR=""; C_DIM=""; C_OFF=""
fi

step() { printf '\n%s▸ %s%s\n' "$C_STEP" "$1" "$C_OFF"; }
ok()   { printf '  %s✓%s %s\n' "$C_OK" "$C_OFF" "$1"; }
info() { printf '  %s%s%s\n' "$C_DIM" "$1" "$C_OFF"; }
warn() { printf '  %s!%s %s\n' "$C_WARN" "$C_OFF" "$1" >&2; }
die()  { printf '\n%s✗ %s%s\n' "$C_ERR" "$1" "$C_OFF" >&2; exit 1; }

have() { command -v "$1" >/dev/null 2>&1; }

# Print a script's header comment as its --help text: everything from line 2 up
# to the first line that is not a comment (so `set -euo pipefail` and the code
# below it never leak into the output).
usage_from_header() {
  awk 'NR == 1 { next }
       /^#/ { sub(/^# ?/, ""); print; next }
       { exit }' "$1"
}

# version_ge 1.82.0 1.79.0  → true when $1 >= $2 (numeric, dot-separated).
version_ge() {
  [ "$1" = "$2" ] && return 0
  # sort -V is present on macOS (BSD) and GNU alike.
  [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" = "$2" ]
}

rust_version() { rustc --version 2>/dev/null | awk '{print $2}' | cut -d- -f1; }
node_version() { node --version 2>/dev/null | sed 's/^v//'; }

# ask "Question?" → 0 on yes. Non-interactive (no TTY) or ASSUME_YES=1 → yes.
ask() {
  if [ -n "${ASSUME_YES:-}" ] || [ ! -t 0 ]; then
    info "$1 → yes (non-interactive)"
    return 0
  fi
  printf '  %s [y/N] ' "$1"
  read -r reply
  case "$reply" in [yY] | [yY][eE][sS]) return 0 ;; *) return 1 ;; esac
}

# The OS family this script is running on: macos | linux | windows | unknown.
detect_os() {
  case "$(uname -s 2>/dev/null)" in
    Darwin) echo macos ;;
    Linux) echo linux ;;
    MINGW* | MSYS* | CYGWIN*) echo windows ;;
    *) echo unknown ;;
  esac
}

# Make a freshly-installed rustup/homebrew visible to the *current* shell, so a
# single run of the setup script can install a tool and then use it.
load_toolchains() {
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  for brew_bin in /opt/homebrew/bin/brew /usr/local/bin/brew; do
    if [ -x "$brew_bin" ] && ! have brew; then
      eval "$("$brew_bin" shellenv)"
    fi
  done
  return 0
}

# Verify everything needed to build and run. Prints what is missing and returns
# non-zero; callers decide whether to install or to instruct.
check_prereqs() {
  local missing=0

  if have rustc && have cargo; then
    local rv
    rv="$(rust_version)"
    if version_ge "$rv" "$MIN_RUST"; then
      ok "Rust $rv"
    else
      warn "Rust $rv is too old — $MIN_RUST+ is required for wasm32-wasip2 (run: rustup update)"
      missing=1
    fi
  else
    warn "Rust (rustc/cargo) not found"
    missing=1
  fi

  if have rustup; then
    if rustup target list --installed 2>/dev/null | grep -q '^wasm32-wasip2$'; then
      ok "wasm32-wasip2 target"
    else
      warn "wasm32-wasip2 target not installed (run: rustup target add wasm32-wasip2)"
      missing=1
    fi
  else
    warn "rustup not found — the wasm32-wasip2 target cannot be checked/installed"
    missing=1
  fi

  if have node && have npm; then
    local nv
    nv="$(node_version)"
    if version_ge "$nv" "$MIN_NODE"; then
      ok "Node $nv"
    else
      warn "Node $nv is too old — $MIN_NODE+ required"
      missing=1
    fi
  else
    warn "Node/npm not found"
    missing=1
  fi

  return $missing
}
