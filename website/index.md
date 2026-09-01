---
title: "Quicktys — stream automation, rebuilt in Rust"
titleTemplate: false
description: "Stream automation for OBS, Twitch and everything around them. A Rust and Tauri desktop app where every integration is a sandboxed WebAssembly extension."
layout: home

hero:
  name: Quicktys
  text: The code anyone can publish is the code that can do the least.
  tagline: Stream automation for OBS, Twitch and everything around them — rebuilt in Rust, where every integration is a sandboxed WebAssembly extension instead of a script with the run of your machine.
  image:
    src: /logo.svg
    alt: Quicktys
  actions:
    - theme: brand
      text: Get started
      link: /guide/what-is-quicktys
    - theme: alt
      text: Write an extension
      link: /dev/overview
    - theme: alt
      text: GitHub
      link: https://github.com/Ascol57/Quicktys
---

<div class="qt-status">
  <div class="qt-status-inner">
    <strong>Where the project stands.</strong>
    Quicktys is a clean-room rewrite of
    <a href="https://github.com/LordTocs/CastMate">CastMate</a>, built in the open
    and still pre-1.0. The host kernel, both extension ABIs, the automation
    runner, the overlay system, the marketplace and the signing chain are
    implemented and covered by integration tests; live Twitch chat, EventSub, OBS
    control and audio all work end to end. Not done yet: release packaging, the
    mass port of the remaining integrations, and the online activation service.
    The <a href="/Quicktys/contributing/roadmap">roadmap</a> gives the honest
    state of every milestone.
    <br><br>
    <strong>The repository is not open yet.</strong> The licence texts are with a
    lawyer before the code goes public, so links to source files on this site,
    and cloning, do not work for the moment. Everything else here is complete and
    accurate — it documents the code as it stands today.
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">The extension model</span>
  <h2>Three kinds of extension, three levels of trust</h2>
  <p class="qt-lead">
    Most automation tools have one extension type and one privilege level:
    whatever the app can do, an add-on can do. Quicktys splits that in three, so
    that the code anybody can publish is the code that can do the least.
  </p>
  <div class="qt-grid">
    <div class="qt-card qt-tier qt-tier-plugin">
      <span class="qt-tier-who">Anyone can write one</span>
      <h3>Plugin</h3>
      <p>
        The part you interact with: actions, triggers, live values, settings.
        Runs in a WebAssembly sandbox with <strong>no network, no files and no
        access to your system</strong> — those interfaces are not in its world at
        all, so it cannot even ask.
      </p>
      <p>
        To reach the outside it calls a module, and the host checks the
        dependency was declared and the data matches the published shape.
      </p>
    </div>
    <div class="qt-card qt-tier qt-tier-module">
      <span class="qt-tier-who">Certified developers, signed</span>
      <h3>Module</h3>
      <p>
        The driver: how to talk to OBS, to Twitch, to your soundcard, to your
        keyboard. The <strong>only</strong> tier that touches the real world.
      </p>
      <p>
        It gets there only if its signature checks out against a pinned
        authority, only for what it declared, and only for what you granted.
        Revoke a permission and it stops working immediately.
      </p>
    </div>
    <div class="qt-card qt-tier qt-tier-widget">
      <span class="qt-tier-who">Anyone can write one</span>
      <h3>Widget</h3>
      <p>
        The pixels: dashboard boxes and stream overlays. No WebAssembly at all —
        ordinary frontend code in a sandboxed frame, fed values through a narrow
        message channel.
      </p>
      <p>
        It can display what it is given and ask the app to run an action. It
        cannot put the machine at risk.
      </p>
    </div>
  </div>

  <p class="qt-wall-note">
    The core ships <strong>zero</strong> modules and zero widgets. Installing a
    plugin works out what drivers it needs, downloads exactly those, and shows you
    what they are asking permission for. Uninstall the plugin and they go with it.
  </p>
</div>

<div class="qt-section">
  <span class="qt-path">The automation engine</span>
  <h2>Automations that branch, loop and wait</h2>
  <div class="qt-split">
  <div>

```yaml
# Start two branches, wait for BOTH, then continue.
- kind: parallel
  join: all
  branches:
    - kind: block
      body:
        - { kind: action, plugin: obs, action: start-record }
        - { kind: wait-for, wait: delay, ms: "3000" }
    - kind: action
      plugin: sound
      action: play
      config: { file: intro.mp3, volume: 80 }

- kind: if
  cond: 'obs.scene == "Intro"'
  then:
    kind: action
    plugin: twitch
    action: chat
    config: { message: "Live with @pear.title" }
```

  </div>
  <div>

<p>
  An automation is a tree of typed nodes run by a tree-walking interpreter in the
  host: <code>action</code>, <code>block</code>, <code>parallel</code>,
  <code>timeline</code>, <code>if</code>, <code>switch</code>, <code>loop</code>,
  <code>let</code>/<code>set</code>, <code>try</code>, <code>wait-for</code>,
  <code>return</code>.
</p>
<p>
  Concurrency is <strong>structured</strong>. A parallel node's join policy
  decides whether to wait for every branch or take the first and cancel the
  losers through a propagated cancellation token. Loops are bounded, waits have
  timeouts, and nothing runs unbounded.
</p>
<p>
  Conditions and values are sandboxed <strong>Rhai</strong> expressions evaluated
  host-side on an already-materialised context — synchronous, deterministic, and
  a syntax tree rather than opaque JavaScript. That is precisely what makes
  dependency tracking, edit-time validation and autocompletion possible at all.
</p>
<p>
  <a href="/Quicktys/reference/nodes">Node reference →</a>
</p>

  </div>
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">Design decisions</span>
  <h2>Three more decisions worth knowing about</h2>
  <div class="qt-grid">
    <div class="qt-card qt-card-mint">
      <h3>Translations are data, not code</h3>
      <p>
        The core, every plugin, every module and every widget carries its own
        <code>lang/</code> folder of per-locale JSON. Manifests emit <em>keys</em>,
        never translated strings, and the host discovers and namespaces each
        folder automatically.
      </p>
      <p>
        Because locales travel next to the <code>.wasm</code> instead of being
        compiled into it, adding a language means adding a file — no build, no
        Rust, no pull request into someone's bundler config.
      </p>
    </div>
    <div class="qt-card qt-card-clay">
      <h3>Overlays are WYSIWYG for real</h3>
      <p>
        Place widgets on a fixed-ratio canvas with snapping guides and
        percentage geometry, then point an OBS browser source at the local
        server. Rendering is identical in the app and on stream.
      </p>
      <p>
        The previews in the editor are not mockups: each one is the actual widget
        rendered in its own sandboxed document, fed the same config message the
        live overlay gets — made inert so it can never swallow a drag.
      </p>
    </div>
    <div class="qt-card qt-card-violet">
      <h3>Paid extensions were designed for on day one</h3>
      <p>
        A signed static registry you can self-host, offline-verifiable licences,
        per-capability consent that takes effect immediately, and hybrid
        post-quantum signature chains.
      </p>
      <p>
        A third-party extension may be closed and commercial — that is exactly
        what the licence exception buys. The verification side stays open: you
        can always audit what your app decides to trust.
      </p>
    </div>
  </div>
</div>

<div class="qt-section">
  <span class="qt-path">In practice</span>
  <h2>What it looks like</h2>
  <p class="qt-lead">
    A tabbed desktop app: a customisable dashboard of widgets, profiles binding
    triggers to sequences, a node editor that highlights the step currently
    running, a browser for every reactive variable, and an overlay editor whose
    previews are the real thing.
  </p>
  <Screenshot src="/screens/hero.png" caption="The dashboard, with live widgets" />
</div>

<div class="qt-section">
  <span class="qt-path">Start here</span>
  <h2>Pick a path</h2>
  <div class="qt-grid">
    <div class="qt-card">
      <h3><a href="/Quicktys/guide/installation">I want to use it</a></h3>
      <p>
        Install per OS, build the bundles, run the app, and wire your first
        trigger to a sequence in about ten minutes.
      </p>
    </div>
    <div class="qt-card">
      <h3><a href="/Quicktys/dev/overview">I want to extend it</a></h3>
      <p>
        Write a plugin in Rust against a WIT world, compile it to a WebAssembly
        component, bundle it with its locales, sign it, publish it.
      </p>
    </div>
    <div class="qt-card">
      <h3><a href="/Quicktys/contributing/architecture">I want to hack on the core</a></h3>
      <p>
        Sixteen host crates, a wasmtime runtime, a Vue 3 frontend, and an
        architecture document that records every structural decision.
      </p>
    </div>
  </div>
</div>
