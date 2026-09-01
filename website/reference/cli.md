---
description: "Quicktys command-line tools: build-bundles, gen-keys, run, quicktys-pack, quicktys-regserve and quicktys-migrate, with every flag and environment variable."
---

# Command-line tools

## `scripts/build-bundles.sh`

Compiles every extension and assembles the bundle directories the host loads.

```bash
scripts/build-bundles.sh [output-dir]      # default: dist/bundles
```

Produces:

```
dist/bundles/
├── modules/<id>/{<id>.wasm, lang/, ui/, bundle.json}
├── plugins/<id>/{<id>.wasm, lang/, ui/, bundle.json}
└── widgets/<id>/{bundle.json, lang/, *.html}
```

**What ships is the `EXTENSIONS` and `WIDGETS` lists at the top of the script.**
Add yours there:

```bash
EXTENSIONS=(
  # kind:dir:id:artifact[:certified]
  "module:modules/mydriver:mydriver:mydriver.wasm:certified"
  "plugin:plugins/mything:mything:mything.wasm"
)
WIDGETS=(
  # kind:dir:id
  "widget:widgets/mybox:mybox"
)
```

The trailing `certified` flag is the development stand-in for a signature — it is
what lets a module hold system capabilities locally.

::: warning The artifact name is not the package name
Cargo replaces `-` with `_`: package `counter-client` → `counter_client.wasm`.
:::

## `setup/gen-keys.sh`

Generates the development keys. `dist/` is gitignored, so a fresh clone has none.

```bash
setup/gen-keys.sh            # create what is missing, never overwrite
setup/gen-keys.sh --show     # inventory only
setup/gen-keys.sh --force    # regenerate everything (invalidates existing signatures)
```

| File | Role |
| --- | --- |
| `dist/ca.key` / `.pub` | development certification authority — signs publisher certificates, which is what makes a module certified. Pinned via `QUICKTYS_CA`. |
| `dist/store.key` / `.pub` | store key — signs entitlements for paid extensions. Pinned via `QUICKTYS_STORE`. |
| `dist/secrets/<kind>-<id>.ck` | content key of an encrypted package, created at packing time |
| `dist/pear.license` | a licence signed by the store key, to paste into the app |

ML-DSA (post-quantum) keypairs, stored as hex. Nothing here is a production
secret; losing `dist/` costs nothing. Never commit it.

## `setup/run.sh`

Builds everything and runs it.

```bash
setup/run.sh                  # registry server + app (default)
setup/run.sh --solo           # the app alone, loading dist/bundles
setup/run.sh --web            # the frontend alone, in a browser (Vite, IPC stubbed)
setup/run.sh --skip-build     # relaunch without rebuilding
setup/run.sh --port 9000      # registry port
```

Windows: `powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1`.

The default mode delegates to `dev.sh`, which packs and **signs** the catalogue
before serving the registry — so the default path exercises the real marketplace
chain.

## `quicktys-pack`

Turns bundle directories into distributable `.qtpkg` packages, signs them, and
issues licences.

```bash
quicktys-pack <bundles_dir> [--out <dir>] [--sign] [--ca <keyfile>] [--paid <kind/id[:sku]>]
quicktys-pack <pkgs_dir> --sign-only [--ca <keyfile>]
quicktys-pack --issue-license <kind/id[:sku]> [--store <keyfile>]
```

| Flag | |
| --- | --- |
| `--out <dir>` | where to write the packages |
| `--sign` | sign each package with the CA key |
| `--ca <keyfile>` | the CA key to sign with (default `dist/ca.key`) |
| `--sign-only` | sign already-built packages in place |
| `--paid <kind/id[:sku]>` | mark a package as paid — the installer will refuse it without a covering licence |
| `--encrypt <kind/id[:sku]>` | encrypt the `.wasm` payload; writes the content key to `dist/secrets/` |
| `--issue-license <kind/id[:sku]>` | mint an entitlement signed by the store key and print it |
| `--store <keyfile>` | the store key to sign licences with |

```bash
scripts/build-bundles.sh
quicktys-pack dist/bundles --out dist/pkgs --sign
quicktys-pack --issue-license module/pear --store dist/store.key > dist/pear.license
```

## `quicktys-regserve`

Serves a directory of packages as a registry.

```bash
quicktys-regserve <packages_dir> [--port N] [--addr IP] [--out DIR] [--sign] [--static]
```

| Flag | Default | |
| --- | --- | --- |
| `--port` | 8000 | listening port |
| `--addr` | 127.0.0.1 | bind address |
| `--out` | a temp dir | where the generated registry tree is written |
| `--sign` | off | sign the generated index |
| `--static` | off | generate the tree and exit, without serving |

Point clients at it:

```bash
QUICKTYS_REGISTRY=http://127.0.0.1:8000 cargo run -p quicktys-app
```

The registry is a static tree — `index.json` (+ `.sig`), `crl.json`, and
content-addressed blobs — so `--static` output can be dropped on any file host.

## `quicktys-migrate`

Best-effort one-way migration from a CastMate data directory.

```bash
quicktys-migrate <castmate-data-dir> [--out <dir>]
```

It reads **your data** — YAML files, not code — and writes Quicktys resources
that are valid by construction, plus a **Markdown report**.

What it does:

- imports profiles, automations and settings;
- maps the automation model (instant / time / stack / flow → the node tree);
- converts a template `{{ simple.path }}` **on its own** into `@simple.path`;
- extracts defensively, tolerating several spellings of a field.

What it deliberately does **not** do:

- **no automatic JavaScript → Rhai translation.** CastMate conditions and filters
  are arbitrary JS; converting them would be unreliable, so they are left for you
  to rewrite and are listed in the report.
- anything ambiguous — unrecognised actions, non-trivial `{{ }}` expressions — is
  **reported, never guessed**.

The report is the entry point: read it first.

## Environment variables

| Variable | |
| --- | --- |
| `QUICKTYS_REGISTRY` | registry base URL |
| `QUICKTYS_CA` | pinned CA public key |
| `QUICKTYS_STORE` | pinned store public key |
| `QUICKTYS_NO_DEV_BUNDLES` | skip scanning `dist/bundles`, so only installed extensions load — this is what `dev.sh` sets to exercise the real install path |
| `HOME` | the data directory is built from it; **must be set on Windows**, which `run-windows.ps1` does |

## Cargo

```bash
cargo check                              # the whole host workspace
cargo build
cargo test -p quicktys-core
cargo test -p quicktys-core some_test
cargo test -p quicktys-host              # the ABI integration suites

# An extension — from its own directory
cd plugins/time && cargo build --target wasm32-wasip2 --release
```

Extensions are **excluded** from the host workspace. Never add one to `members`.

## Next

- [Package & publish](/dev/packaging)
- [Files & data directory](/reference/files)
