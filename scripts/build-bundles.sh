#!/usr/bin/env bash
# Build Quicktys extension bundles.
#
# Each extension is a standalone wasm32-wasip2 package (own Cargo.lock, excluded
# from the host workspace). This compiles each to a Component and assembles a
# self-contained *bundle* directory the host can load without any hardcoded dev
# path: the `.wasm`, its `lang/` locales, and a `bundle.json` descriptor.
#
# Layout produced (BUNDLES defaults to dist/bundles):
#   dist/bundles/plugins/<id>/{<id>.wasm, lang/, bundle.json}
#   dist/bundles/modules/<id>/{<id>.wasm, lang/, bundle.json}
#
# Usage: scripts/build-bundles.sh [output-dir]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/dist/bundles}"
TARGET="wasm32-wasip2"

# kind : crate dir : id : wasm artifact (package name, '-' -> '_') [ : certified ]
#
# `time` is intentionally omitted: its `delay` action is a pure ABI fixture that
# does NOT actually wait (real delays are the runner's `wait-for` node), so
# auto-loading it would mislead. Add it here if you want it in the palette.
EXTENSIONS=(
  "module:modules/pear:pear:pear.wasm:certified"
  "plugin:plugins/pear:pear:pear_plugin.wasm"
  "plugin:plugins/notify:notify:notify.wasm"
  "plugin:plugins/alert:alert:alert.wasm"
  "module:modules/obs:obs:obs_module.wasm:certified"
  "plugin:plugins/obs:obs:obs_plugin.wasm"
  "module:modules/webview:webview:webview_module.wasm"
  "module:modules/http:http:http_module.wasm:certified"
  "plugin:plugins/discord:discord:discord_plugin.wasm"
  "module:modules/sound:sound:sound_module.wasm:certified"
  "plugin:plugins/sound:sound:sound_plugin.wasm"
  "module:modules/os:os:os_module.wasm:certified"
  "plugin:plugins/os:os:os_plugin.wasm"
  "module:modules/input:input:input_module.wasm:certified"
  "plugin:plugins/input:input:input_plugin.wasm"
  "module:modules/twitch:twitch:twitch.wasm:certified"
  "plugin:plugins/twitch:twitch:twitch_plugin.wasm"
  "plugin:plugins/random:random:random.wasm"
  "plugin:plugins/countdown:countdown:countdown.wasm"
)

# Widgets are frontend-only (no WASM): the source dir already IS the bundle
# (bundle.json + lang), so bundling is just a copy.  kind:dir:id
WIDGETS=(
  "widget:widgets/info:info"
  "widget:widgets/button:button"
  "widget:widgets/pear-now-playing:pear-now-playing"
  "widget:widgets/pear-queue:pear-queue"
  "widget:widgets/pear-music:pear-music"
  "widget:widgets/alert:alert"
  "widget:widgets/countdown:countdown"
)

bundle_one() {
  local kind="$1" dir="$2" id="$3" artifact="$4" flag="${5:-}"
  local crate="$ROOT/$dir"
  echo "→ building $kind '$id' ($dir)"
  (cd "$crate" && cargo build --target "$TARGET" --release >/dev/null)

  local src="$crate/target/$TARGET/release/$artifact"
  if [[ ! -f "$src" ]]; then
    echo "  ✗ missing artifact: $src" >&2
    return 1
  fi

  local dest="$OUT/${kind}s/$id"
  rm -rf "$dest"
  mkdir -p "$dest"
  cp "$src" "$dest/$id.wasm"
  [[ -d "$crate/lang" ]] && cp -r "$crate/lang" "$dest/lang"
  # Custom slot UI files a plugin ships (served over `pluginui://<id>/…`).
  [[ -d "$crate/ui" ]] && cp -r "$crate/ui/." "$dest/"

  local certified="false"
  [[ "$flag" == "certified" ]] && certified="true"
  cat >"$dest/bundle.json" <<JSON
{
  "id": "$id",
  "kind": "$kind",
  "wasm": "$id.wasm",
  "certified": $certified
}
JSON
  echo "  ✓ $dest"
}

# A frontend-only widget: the source dir IS the bundle (bundle.json, optional
# index.html for a runtime widget, lang/). Copy it as-is.
bundle_widget() {
  local kind="$1" dir="$2" id="$3"
  local srcdir="$ROOT/$dir"
  echo "→ packaging $kind '$id' ($dir)"
  local dest="$OUT/${kind}s/$id"
  rm -rf "$dest"
  mkdir -p "$dest"
  cp -r "$srcdir/." "$dest/"
  echo "  ✓ $dest"
}

for spec in "${EXTENSIONS[@]}"; do
  IFS=':' read -r kind dir id artifact flag <<<"$spec"
  bundle_one "$kind" "$dir" "$id" "$artifact" "${flag:-}"
done

for spec in "${WIDGETS[@]}"; do
  IFS=':' read -r kind dir id <<<"$spec"
  bundle_widget "$kind" "$dir" "$id"
done

echo "bundles → $OUT"
