# Package & publish

Getting an extension from a directory on your disk to something someone else can
install.

## The `.qtpkg` format

A bundle is a directory. For transport it is serialised into a single
**content-addressed** artefact:

```
<id>-<version>.qtpkg          deterministic tar + zstd of the bundle directory
├── manifest.json             the extended descriptor
├── <id>.wasm                 absent for a widget or content package
├── lang/…                    locales
├── index.html                runtime widgets only
└── signature.json            mandatory for modules, optional otherwise
```

The archive is **deterministic** — sorted entries, fixed permissions and
timestamps — so the same content produces the same bytes, and therefore the same
`sha256`. That hash is the package's identity, its cache key and its CDN key.

### `manifest.json`

A superset of the `bundle.json` the host reads at load time:

```jsonc
{
  "id": "obs",
  "kind": "module",                 // plugin | module | widget | content
  "version": "1.4.0",               // semver
  "wasm": "obs.wasm",               // absent for widget / content
  "requires": [                     // plugins: the modules they need
    { "module": "http", "version": "^1" }
  ],
  "capabilities": ["net"],          // modules: requested system capabilities
  "min_host": "0.3.0",
  "publisher": "acme",
  "license": { "model": "free" },   // free | paid | subscription
  "hashes": { "obs.wasm": "sha256:…" }
}
```

Per-file hashes are defence in depth: the signature commits to the manifest, and
the manifest commits to every file.

::: info Ids are not identity
Two publishers may both publish `id: "obs"`. What distinguishes them is the
`publisher` field and the signature chain, not the id. The installer refuses to
overwrite an installed extension from a **different publisher** without explicit
confirmation.
:::

## Packing

```bash
quicktys-pack <bundles_dir> [--out <dir>] [--sign] [--ca <keyfile>]
quicktys-pack <bundles_dir> --paid <kind/id[:sku]>
quicktys-pack <bundles_dir> --encrypt <kind/id[:sku]>
quicktys-pack <pkgs_dir> --sign-only [--ca <keyfile>]
quicktys-pack --issue-license <kind/id[:sku]> [--store <keyfile>]
```

The usual development cycle:

```bash
scripts/build-bundles.sh                       # dirs → dist/bundles/
quicktys-pack dist/bundles --out dist/pkgs --sign
quicktys-regserve dist/pkgs --port 8000
QUICKTYS_REGISTRY=http://127.0.0.1:8000 cargo run -p quicktys-app
```

`setup/run.sh` does exactly this by default, which is why the default development
path exercises the real marketplace chain rather than bypassing it.

## Signing

Provenance is a chain: **certification authority → publisher certificate →
release**, each link signed. The pair is hybrid post-quantum (Ed25519 + ML-DSA).

```jsonc
// signature.json
{ "alg": "ed25519", "content_hash": "…", "sig": "…", "publisher_cert": { /* …, ca_sig */ } }
```

At install time the whole chain is verified against **pinned CA keys** plus a
revocation list, producing one of three verdicts:

| Verdict | Consequence |
| --- | --- |
| **Certified** | valid chain, signed manifest hash, declared capabilities within the certificate's ceiling, not expired, not revoked |
| **Unsigned** | installs, but is **not** certified — a module's system capabilities are refused at load |
| **Rejected** | an invalid signature **refuses** the install outright |

`certified` is therefore *computed by cryptography*, never read as a boolean from
a file someone can edit.

::: info Why the signature covers the manifest, not the archive
`signature.json` lives *inside* the archive, so signing the whole-archive hash
would be circular. The signature commits to the **manifest hash**; the manifest
covers the payload through its per-file `hashes`.
:::

### Development keys

`setup/gen-keys.sh` generates a local CA (`dist/ca.key` / `.pub`) and a store key
(`dist/store.key` / `.pub`), idempotently. Pin them with `QUICKTYS_CA` and
`QUICKTYS_STORE`. None of it is a production secret — losing `dist/` costs
nothing.

With an **empty trust store** (no CA operations yet) development bundles are
trusted by locality, which is what makes `--solo` work.

## Certification

Only certified modules may hold system capabilities. Certification means:

1. a review of the module,
2. a publisher certificate issued by the Quicktys authority,
3. a **capability ceiling** on that certificate — you cannot later sign a release
   asking for more than you were certified for,
4. signing each release with your publisher key.

Plugins and widgets need none of this. Their sandbox is the guarantee, and a
plugin cannot request a system capability at all.

::: warning The certification programme is not open yet
The signing infrastructure exists and is exercised end to end by the development
keys, but the authority, the review process and key revocation operations are
still being set up. Watch the [roadmap](/contributing/roadmap).
:::

## The registry

A registry is **static** — no application server, nothing dynamic:

```
registry/
├── index.json            signed catalogue, one entry per published version
├── index.sig
├── crl.json (+ .sig)     revocation list
└── blobs/
    └── sha256/<hash>.qtpkg
```

Serve it from a bucket, a CDN, or `quicktys-regserve`. Three reasons for the
shape: a minimal server is a minimal attack surface; it is trivially
self-hostable; and trust rests on **package signatures**, not on the server.

A compromised index can at worst hide packages or lie about metadata. It cannot
fabricate a validly certified module.

```bash
quicktys-regserve <packages_dir> --port 8000 --addr 127.0.0.1 --sign
```

Point clients at it with `QUICKTYS_REGISTRY`.

## Versions and dependencies

One structural constraint shapes everything: **only one version of a given module
id can be loaded at a time.** The loader deduplicates by id, so two plugins
requiring incompatible versions of `obs` cannot coexist.

The resolver therefore produces **one** resolved version per id that satisfies
**every** constraint, walking dependencies depth-first, deps before dependants,
with cycle detection.

Practical consequence for module authors: **semver discipline is not optional.**
A breaking change to a service's arguments is a major version, and shipping one
quietly will break plugins that are not yours.

## Paid extensions

A package declares its licence model in the manifest. `paid` and `subscription`
packages will not install without a covering **licence**.

A licence is a `QTLIC-…` key: an entitlement **signed by the store** and verified
**offline** against pinned store keys. Once the user has it, no network is
needed.

```bash
quicktys-pack --issue-license module/pear:pro --store dist/store.key
```

### Encrypted payloads

`--encrypt` produces `<wasm>.enc` — the component encrypted with
XChaCha20-Poly1305 — and declares it in `manifest.protection`. The content key is
delivered **inside the licence**, so there is no account and no server in the
path.

Decryption happens **at load time, in memory**. The plaintext is never written
back to disk, at boot or at hot-load.

Signing and encryption stay **independent layers**. A premium package is signed
in the clear — so it remains catalogable and its certification computable — *and*
encrypted.

### Watermarking

A package may be marked `watermark: true`. At install time, an opaque per-buyer
id is woven invisibly into the package's **text assets** using zero-width
characters, repeated so it survives truncation.

It is applied **after** provenance verification (a mark cannot be part of the
signed bytes) and **never** to `*.json` (it would break the signature and the
manifest). If a copy leaks, the id can be extracted and mapped back to a buyer
server-side.

::: warning This is deterrence, not protection
The design says so plainly: a client-side watermark can be removed, and a perfect
client-side DRM does not exist. The goal is to protect the honest majority and to
be able to trace and revoke — not to be unbreakable. The decryptor is open by
design; the secret is in the keys.
:::

## Sideloading

A `.qtpkg` can be installed from disk. It goes through identical verification: an
unsigned package installs as uncertified, and an uncertified module that declares
capabilities is refused at load. Sideloading buys no privileges.

## Checklist before publishing

- [ ] The manifest emits **i18n keys**, not English strings.
- [ ] `lang/en.json` covers every key, and is the reference locale.
- [ ] The version is semver, and a breaking service change bumped the major.
- [ ] A module declares only the capabilities it actually uses.
- [ ] A network driver exposes a `set-base-url`-style service so it can be tested
      offline.
- [ ] Widget booleans are named `showX`, never `hideX`.
- [ ] Enum option labels live under `options.<key>.<value>`.
- [ ] Your licence is stated in `Cargo.toml` and in the package manifest.

## Next

- [Command-line tools](/reference/cli) — every flag.
- [Marketplace](/guide/marketplace) — the installing side.
