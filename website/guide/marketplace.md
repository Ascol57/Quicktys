---
description: "Install plugins and widgets from a signed Quicktys registry: dependency resolution, signature verification, capability consent, updates and paid licences."
---

# Marketplace

The Marketplace tab is where you get plugins and widgets. It talks to a
**registry** — a signed static index that can be served from a bucket, a CDN, or
your own machine.

<Screenshot src="/screens/marketplace.png" caption="The Marketplace" />

## What you can browse

**Plugins and widgets only.** Modules are deliberately not browsable: a module is
a driver, and there is no reason to install one on its own. Modules arrive as a
plugin's declared dependency, and leave when nothing needs them any more.

That is why the overlay system is fronted by an **Overlays** plugin even though
the real work is in the `webview` module — without a plugin requiring it, the
module could never be installed at all.

## Installing

Picking a plugin runs a small state machine:

1. **Resolve.** The registry works out the full dependency set — the plugin plus
   the certified modules it declares (`obs@^1`), depth-first, refusing cycles.
2. **Download.** Each package is fetched by content hash and verified against it.
3. **Verify.** Every package's signature chain is checked: authority → publisher
   certificate → release, each link signed, against pinned CA keys and a
   revocation list. An invalid signature **refuses** the install. An unsigned
   package installs but stays *uncertified* — which means its system capabilities
   are refused at load.
4. **Consent.** If a module asks for system capabilities, you are shown what and
   why before anything is granted.
5. **Install.** Files are extracted atomically into `installed/`, and the
   extensions are hot-loaded — modules first, then the plugins that depend on
   them. No restart.

<Screenshot src="/screens/marketplace-consent.png" caption="The capability consent screen" />

## Certification and consent are two different things

This trips people up, so plainly:

- **Certification** decides whether a module *may* touch a capability at all. It
  is cryptographic and it is not your decision — the signature either verifies
  against a pinned authority or it does not.
- **Consent** decides whether *you* allow it. It is your decision, stored on your
  machine, and revocable.

Both must hold on **every single call**: `granted = certified && declared &&
consented`. Revoking a capability takes effect immediately, not at next restart.

An update that **widens** what a module asks for has its new capabilities refused
until you review them again.

::: info Grace until review
A module you have never reviewed is allowed what it declared, so installing
something does not break before you have opened the consent screen. Once you have
reviewed it, only what you granted is allowed.
:::

## Updates and removal

**Updates** are listed per extension; the same verification and consent rules
apply to the new version.

**Removing** a plugin drops the modules that were pulled in only as its
dependencies. Modules installed as a dependency are marked non-explicit, which is
what makes orphan cleanup possible. A module something else still needs stays.

## Paid extensions

A package carries a licence model: free, paid, or subscription. A paid package
will not install without a **licence** covering it.

A licence is a key (`QTLIC-…`) that is an entitlement **signed by the store** and
verified **offline** against pinned store keys. Once you have it, no network is
needed — the app keeps working on a plane.

Some paid packages ship their WebAssembly **encrypted**. The content key is
delivered inside the licence itself, and decryption happens **at load time, in
memory** — the plaintext is never written back to disk. Signing and encryption are
independent layers: a premium package is signed in the clear (so it remains
catalogable and its certification computable) *and* encrypted.

Paste a licence key in **Settings → Licences**.

## Running your own registry

The registry is a static `index.json` (signed) plus `crl.json` plus
content-addressed `.qtpkg` blobs. There is nothing dynamic about it, so any file
host works.

```bash
quicktys-regserve <packages_dir> --port 8000 --sign
```

Point the app at it with the `QUICKTYS_REGISTRY` environment variable. In
development, `setup/run.sh` does exactly this — it packs and signs the local
catalogue, then serves it, so the full marketplace chain is exercised rather than
bypassed.

See [Package & publish](/dev/packaging) for the publishing side, and
[Command-line tools](/reference/cli) for the flags.

## Sideloading

You can install a `.qtpkg` from disk. It goes through the same verification: an
unsigned package installs as uncertified, and an uncertified module that declares
capabilities is **refused at load**. Sideloading does not buy you privileges.

## Next

- [Connecting accounts](/guide/accounts) — after installing Twitch.
- [Package & publish](/dev/packaging) — the other side of the counter.
