#!/usr/bin/env bash
# Quicktys — setup entry point. Detects the OS and runs the right script.
#
#   ./setup/setup.sh            # install prerequisites for this machine
#   ASSUME_YES=1 ./setup/setup.sh
#
# Then:  ./setup/run.sh
set -euo pipefail
. "$(cd "$(dirname "$0")" && pwd)/_common.sh"

case "$(detect_os)" in
  macos) exec "$REPO_ROOT/setup/setup-macos.sh" "$@" ;;
  linux) exec "$REPO_ROOT/setup/setup-linux.sh" "$@" ;;
  windows)
    die "Running under Git Bash/MSYS on Windows — use PowerShell instead:
       powershell -ExecutionPolicy Bypass -File setup\\setup-windows.ps1"
    ;;
  *) die "Unsupported OS: $(uname -s). See setup/README.md." ;;
esac
