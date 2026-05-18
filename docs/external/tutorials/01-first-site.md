# Tutorial 01: Your first PlausiDen site

> **Diátaxis tier: Tutorial** — learning-oriented. We'll build a
> working three-page site together, end-to-end, in about 15
> minutes. Every step exists to teach a concept; not every step
> reflects the fastest way to do this in practice.

By the end of this tutorial you will have:

* a `manifest.toml` declaring your platform capabilities
* a `forge.toml` with build configuration
* three rendered pages
* a passing `forge build` + `forge manifest validate`
* a deploy bundle ready to upload

## Prerequisites

* Rust 1.78 or newer (`rustup show` should print `1.78.0` or
  higher).
* A clone of `PlausiDen-Forge` somewhere on disk.
* About 250 MB of free disk space for the build cache.

## Step 1: Scaffold

In an empty directory, create the workspace shell.

```bash
mkdir my-first-site && cd my-first-site
```

Add a `manifest.toml`:

```toml
platform = "my-first-site"
schema-version = "1"

[[capabilities]]
id = "home-page"
summary = "Landing page"
ownership = "forge"
```

This is the smallest legal [PlatformManifest](../reference/manifest-toml.md).
One capability, one stable identifier, one declared ownership.

## Step 2: Add a phase pipeline

Create `phases.toml`:

```toml
schema-version = 1

[phases.tokens]
summary          = "Loom token surface presence + minimum coverage check"
default-severity = "warn"

[phases.html-semantic]
summary          = "Sectioning + heading hierarchy + landmark rules"
default-severity = "strict"
```

This declares the two phases Forge will run on every build. The
phases are evaluated in topological order — `tokens` first
because nothing depends on it.

## Step 3: Add a page

Create `cms/home.toml`:

```toml
slug = "home"
title = "Hello, world"
description = "A demonstration of PlausiDen's typed CMS layer."
status = "draft"

[[sections]]
anchor = "hero"
theme = "light"

  [[sections.blocks]]
  kind = "hero"
  fields.text = "Welcome to my first PlausiDen site."

  [[sections.blocks]]
  kind = "cta"
  fields.text = "Get started"
  fields.url = "/start"
```

## Step 4: Validate the manifest

```bash
cargo run -q -p forge-cli -- manifest validate
```

You should see:

```
manifest-gate: 0 backends (0 stub), 2 phases, topo ok
```

If the topo check fails or any backend/phase id is invalid, the
command exits non-zero and prints exactly which id failed and why.

## Step 5: Run the build

```bash
cargo run -q -p forge-cli
```

Forge walks the phase pipeline + emits a build report under
`reports/build-{timestamp}.json`. Each phase contributes typed
findings; the gate fails on any `strict` finding.

## Step 6: Inspect the report

The build report is JSON with a typed `findings` array. Open the
most recent file:

```bash
ls -t reports/ | head -1
```

Each entry has `phase`, `severity`, `kind`, `detail`. The kind
strings are stable identifiers you can grep for, e.g.
`html-semantic.missing-h1`.

## Where to go next

* **Add a deploy target** — see [How-to: deploy to Tor](../how-to/deploy-to-tor.md).
* **Understand the manifest** — see
  [Reference: manifest.toml](../reference/manifest-toml.md).
* **Understand the design** — see
  [Explanation: why typed everything](../explanation/why-typed-everything.md).
