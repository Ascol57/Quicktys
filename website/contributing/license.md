---
description: "The four Quicktys licence regimes: an AGPL core with an extension exception, MIT guest SDKs, and proprietary reference extensions and commerce tooling."
---

# Licensing

The repository has **four** licence regimes. Which one applies depends on what
you are looking at.

| What | Where | Licence |
| --- | --- | --- |
| **The Quicktys core** | `crates/` (except below), `frontend/`, `wit/`, `lang/`, `scripts/`, root | **AGPL-3.0-only** + the *Quicktys Extension Exception* |
| **The guest SDKs** | `quicktys-plugin-sdk`, `quicktys-module-sdk` | **MIT** |
| **The reference extensions** | `plugins/`, `modules/`, `widgets/` | **Proprietary, source-available** — © Ascol57 |
| **The commerce tooling** | `quicktys-regserve`, `quicktys-pack` | **Proprietary, source-available** — © Ascol57 |
| **Your third-party extensions** | outside this repository | **whatever you choose**, including closed and commercial |

The authoritative texts are the `LICENSE*` files;
[`COPYING.md`](https://github.com/Ascol57/Quicktys/blob/main/COPYING.md) is the
map.

## The core: AGPL-3.0 + an extension exception

The core is free software under **AGPL-3.0-only** — *network* copyleft. Any
modified version you **distribute** or **expose as a service** (an alternative
marketplace, a hosted backend) must publish its sources. A closed, resold fork is
therefore not possible.

The **extension exception** fixes the one undesirable side effect of that choice.
It guarantees that an extension talking to the core **solely through the official
ABI** is **not a derivative work**. You can write, **sell** and distribute a
closed plugin, module or widget without your code being contaminated.

"Official Extension ABI" is defined restrictively:

- the WIT contracts in `wit/` and any bindings mechanically generated from them;
- the guest SDK crates;
- the host ↔ WebView IPC command and event catalogue;
- the widget bridge — the local transport, the `widget://` scheme, the documented
  `postMessage` protocol;
- the manifest, bundle, schema and `lang/` file formats.

It is the WASM isolation plus the ABI's stability that make this separation
defensible.

**What the exception does not do:**

- it grants nothing regarding the core itself — modifying it stays under the
  AGPL, §13 included;
- it does not cover an extension that **copies core code** or bypasses the ABI to
  reach internals;
- it grants no rights over the "Quicktys" name.

SPDX for core files:

```
AGPL-3.0-only WITH quicktys-extension-exception
```

::: warning A custom exception identifier
`quicktys-extension-exception` is **not** a registered SPDX identifier. A tool
validating SPDX strictly may reject it.
:::

## The guest SDKs: MIT

`quicktys-plugin-sdk` and `quicktys-module-sdk` are **MIT**
([`LICENSE-SDK`](https://github.com/Ascol57/Quicktys/blob/main/LICENSE-SDK)), not
the core licence — because they are **statically linked into every extension**.

The exception already listed them as part of the official ABI, so linking them
never made an extension derivative. Licensing them MIT removes the question
instead of answering it by cross-reference, and lets the SDKs be vendored, forked
and reused outside Quicktys entirely.

Practical consequence: **nothing copyleft can reach your extension through the
SDK.**

::: warning A known gap
The SDKs re-export `quicktys_schema::Schema`, so `quicktys-schema` — currently
AGPL — is compiled into every extension too. That is the same problem the
exception solves for the SDKs, but the crate is not named in it.

It needs to be either added to the exception's ABI list or moved to MIT. This is
recorded as an open item in `ARCHITECTURE.md` §0bis. If you are shipping a closed
extension today, be aware of it.
:::

## The extensions in this repository: mine, but visible

The plugins, modules and widgets shipped here are **proprietary works of
Ascol57**. Their source is public — for review, transparency, and because the
core needs them as test fixtures — but **publishing is not giving away**:

- ✅ you may **read**, **build** and **use** them with Quicktys, and modify them
  **privately** for testing;
- ❌ you may not **redistribute**, **fork publicly**, **sell** or **reuse** them in
  another product or another extension.

They remain **reference material**: nothing stops you studying *how* they call the
public ABI and writing your own against that same ABI. What is forbidden is
reusing *this code* to do it.

SPDX in their manifests: `LicenseRef-Ascol57-Proprietary`.

## The commerce tooling: proprietary

The marketplace has two halves, and they are not under the same licence.

**The consumer side stays in the AGPL core:** `quicktys-registry` (search,
resolve, download), `quicktys-cert` and `quicktys-crypto` (verify signature
chains, entitlements, encrypted payloads), `quicktys-install`.

Verification **must** stay open. Users have to be able to audit what their
application decides to trust, and a client-side DRM whose decryptor is secret is
security theatre — `docs/drm.md` §0 says so from the first page: the decryptor is
open by design, the secret is in the keys.

**The issuing side is proprietary:** `quicktys-regserve` (the registry API
server) and `quicktys-pack` (packing, **signing**, minting licences), under the
same terms as the reference extensions.

In short: anyone can verify what the store signs; minting signatures and running
the store is not free.

The AGPL §13 still applies to the **core** — a marketplace *derived from the core*
must publish its sources. The two crates above change nothing about that.

## Contributions: a CLA

Contributions are accepted under a **Contributor License Agreement**: the
contributor assigns or licenses their rights to the owner, who therefore holds
the project and can **dual-license** it — selling commercial licences exempt from
the AGPL.

**The CLA text is not written yet.**

## The clean-room constraint

Quicktys contains **no line of CastMate code** (MPL-2.0). It is a clean-room
reimplementation: the *concepts* and *APIs* were studied, all the code is written
from scratch — the frontend included.

Reading a user's CastMate YAML for migration is fine. That is their data, not
CastMate's code.

## Not legal advice

These texts are a drafting proposal, not legal advice. They have **not been
reviewed by a lawyer**. `ARCHITECTURE.md` §0bis explicitly provides for the
exception and the CLA to be validated by a professional before any commercial
exploitation.

## Next

- [Architecture](/contributing/architecture)
- [Package & publish](/dev/packaging)
