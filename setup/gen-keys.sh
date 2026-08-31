#!/usr/bin/env bash
# Quicktys — generate the local development keys.
#
#   ./setup/gen-keys.sh           # create whatever is missing, never overwrite
#   ./setup/gen-keys.sh --force   # wipe and regenerate (invalidates signatures)
#   ./setup/gen-keys.sh --show    # print what exists, generate nothing
#
# `dist/` is gitignored, so a fresh clone has NO keys: they are per-developer
# material that must be created locally. Two keypairs (post-quantum ML-DSA,
# stored hex-encoded) drive the whole trust chain:
#
#   dist/ca.key    / ca.pub     the development **certification authority**. It
#                               signs publisher certificates, which is what makes
#                               a module "certified" and therefore allowed to hold
#                               system capabilities (net, input, audio, …). The
#                               app pins its public half through QUICKTYS_CA.
#   dist/store.key / store.pub  the **marketplace store key**. It signs licences
#                               (entitlements) for paid extensions; the app pins
#                               its public half through QUICKTYS_STORE.
#
# Two more pieces of key material are produced later, by the packing step in
# dev.sh, and are NOT created here because they depend on the packed catalogue:
#
#   dist/secrets/<kind>-<id>.ck   per-package content key (encrypted/premium
#                                 packages, e.g. module/pear)
#   dist/pear.license             an entitlement signed by the store key
#
# Losing dist/ is harmless: delete it and re-run this script + dev.sh. Nothing
# here is a production secret, and none of it should ever be committed.
set -euo pipefail
. "$(cd "$(dirname "$0")" && pwd)/_common.sh"
cd "$REPO_ROOT"

FORCE=""
SHOW_ONLY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --force) FORCE=1 ;;
    --show) SHOW_ONLY=1 ;;
    -h | --help) usage_from_header "$0"; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
  shift
done

CA_KEY="dist/ca.key"
STORE_KEY="dist/store.key"

show_state() {
  for f in "$CA_KEY" "dist/ca.pub" "$STORE_KEY" "dist/store.pub" "dist/pear.license"; do
    if [ -f "$f" ]; then
      ok "$f ($(wc -c <"$f" | tr -d ' ') bytes)"
    else
      info "$f — missing"
    fi
  done
  if [ -d dist/secrets ] && [ -n "$(ls -A dist/secrets 2>/dev/null)" ]; then
    for f in dist/secrets/*; do ok "$f"; done
  else
    info "dist/secrets/ — empty (content keys are created when packing)"
  fi
}

if [ -n "$SHOW_ONLY" ]; then
  step "Development keys"
  show_state
  exit 0
fi

if [ -n "$FORCE" ]; then
  step "Removing existing keys (--force)"
  warn "packages already signed with the old CA will no longer verify"
  warn "licences issued with the old store key become invalid"
  if ask "Delete dist/ca.key, dist/store.key, dist/secrets/ and dist/pear.license?"; then
    rm -f "$CA_KEY" dist/ca.pub "$STORE_KEY" dist/store.pub dist/pear.license
    rm -rf dist/secrets
    ok "removed"
  else
    die "aborted"
  fi
fi

mkdir -p dist

# The generator lives in the packing tool; build it once.
step "Building the packaging tool (quicktys-pack)"
cargo build -q -p quicktys-pack
ok "built"

# --- Certification authority -------------------------------------------------
# `quicktys-pack` creates the CA on demand as part of a --sign run. Point it at
# an empty directory so it creates the keypair without packing anything.
step "Certification authority (dist/ca.key)"
if [ -f "$CA_KEY" ] && [ -f dist/ca.pub ]; then
  ok "already present — keeping it"
else
  EMPTY="$(mktemp -d)"
  trap 'rm -rf "$EMPTY"' EXIT
  cargo run -q -p quicktys-pack -- "$EMPTY" --out "$EMPTY/out" --sign --ca "$CA_KEY" >/dev/null
  [ -f "$CA_KEY" ] && [ -f dist/ca.pub ] || die "the CA keypair was not created"
  ok "created dist/ca.key + dist/ca.pub"
fi

# --- Store key (+ the sample licence it can sign) ----------------------------
step "Store key (dist/store.key)"
if [ -f "$STORE_KEY" ] && [ -f dist/store.pub ]; then
  ok "already present — keeping it"
else
  # `--issue-license` creates the store keypair on demand and returns before it
  # needs a catalogue, so it doubles as the store-key generator.
  cargo run -q -p quicktys-pack -- --issue-license module/pear:pear-pro \
    --store "$STORE_KEY" 2>/dev/null | grep '^QTLIC-' >dist/pear.license || true
  [ -f "$STORE_KEY" ] && [ -f dist/store.pub ] || die "the store keypair was not created"
  ok "created dist/store.key + dist/store.pub"
  [ -s dist/pear.license ] && ok "issued dist/pear.license (paste it in the app to unlock pear)"
fi

step "Done"
show_state
cat <<EOF

  The app pins these when it runs (dev.sh does it for you):
    QUICKTYS_CA=\$(cat dist/ca.pub)
    QUICKTYS_STORE=\$(cat dist/store.pub)

  Next:  ./setup/run.sh
EOF
